use bevy::prelude::*;
use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;
use std::collections::{BinaryHeap, VecDeque};
use std::cmp::Reverse;

use crate::field;
use crate::grid::Grid;

const RIVER_SEED: u64 = 0xBEDA;

/// Directional cost constants: against-heading steps are penalised by this much
/// on top of the base noise cost (scale ~1–1001). Low bendiness → high penalty
/// (river runs straight); high bendiness → low penalty (river wanders freely).
const PEN_HIGH: u32 = 5000;
const PEN_LOW: u32 = 100;

/// Primary flow direction for the river. Only `Down` (top→bottom) is exercised;
/// the enum is kept minimal so a future `Right` variant is a small addition.
pub enum Heading {
    Down,
}

/// Declarative authoring knobs for the watershed. One `RiverSpec` plus the
/// `RIVER_SEED` fully determines the water field; same pair → same world.
///
/// Could become a Bevy `Resource` later to expose knobs to the editor UI.
pub struct RiverSpec {
    /// Number of main rivers (default 3 — evenly spaced for elk crossings).
    /// Entry columns are distributed across the width as width*(i+1)/(count+1),
    /// so spacing tracks the grid size instead of a fixed column period.
    pub count: usize,
    /// Primary flow direction (default `Down`: top-edge → bottom-edge).
    pub heading: Heading,
    /// Lateral lean: exit_col = entry_col + drift * height (default ~0.25).
    pub drift: f32,
    /// [0, 1] single knob: 0 = nearly straight / broad bends, 1 = wanders far.
    /// Drives both the smoothing-pass count (wavelength) and the directional-
    /// penalty weight — one intuitive control instead of two separate dials.
    pub bendiness: f32,
    /// Shallow feeders that branch off the main channels at a confluence.
    pub tributaries: usize,
    /// Main-channel raster radius (cells from centerline to bank).
    pub radius: isize,
    /// Full-depth core radius (≤ radius; cells within get max water).
    pub core: isize,
    /// BFS reach for the water-proximity carrying-capacity field.
    pub water_reach: u32,
    pub trib_radius: isize,
    pub trib_depth: f32,
    pub oxbow_count: usize,
    pub oxbow_radius: isize,
    pub oxbow_depth: f32,
    pub ford_spacing: usize,
    pub ford_depth: f32,
    /// Fractional jitter applied to each river's drift: drift_i = drift * (1 ± spread).
    pub drift_spread: f32,
    /// Additive jitter on each river's bendiness (clamped to [0, 1]).
    pub bendiness_spread: f32,
    /// Number of large standing-water lakes seeded at the deepest basins.
    pub lake_count: usize,
    /// Raster radius for each lake (cells from center to shore).
    pub lake_radius: isize,
    /// Full-depth core radius for each lake.
    pub lake_core: isize,
}

impl Default for RiverSpec {
    fn default() -> Self {
        Self {
            count: 3,
            heading: Heading::Down,
            drift: 0.25,
            bendiness: 0.5,
            tributaries: 2,
            radius: 3,
            core: 1,
            water_reach: 8,
            trib_radius: 1,
            trib_depth: 0.35,
            oxbow_count: 2,
            oxbow_radius: 1,
            oxbow_depth: 0.3,
            ford_spacing: 20,
            ford_depth: 0.3,
            drift_spread: 0.2,
            bendiness_spread: 0.2,
            lake_count: 3,
            lake_radius: 4,
            lake_core: 2,
        }
    }
}

fn generate_river(mut grid: ResMut<Grid>) {
    let spec = RiverSpec::default();
    generate_river_inner(&mut grid, &spec);
}

/// Generate the full water field from `spec`. Returns `(mains, tribs)`: the
/// carved centerline paths for the main rivers and tributary feeders, which
/// tests use to assert structural invariants. The `generate_river` system
/// discards the return value.
fn generate_river_inner(grid: &mut Grid, spec: &RiverSpec) -> (Vec<Vec<usize>>, Vec<Vec<usize>>) {
    let mut rng = StdRng::seed_from_u64(RIVER_SEED);

    // Smoothing passes from global bendiness — shared cost field for all channels.
    // Per-river directional penalty is derived inside the loop from a child RNG.
    let passes = lerp(8.0, 2.0, spec.bendiness).round() as usize;

    // One shared cost field for all channels — they belong to the same landscape.
    let cost = cost_field(grid, &mut rng, passes);

    let width = grid.width();
    let height = grid.height();

    let mut mains: Vec<Vec<usize>> = Vec::new();
    for i in 0..spec.count {
        // Distribute entry columns evenly across the interior, scale-free: the
        // i-th of `count` rivers enters at width*(i+1)/(count+1), so the spacing
        // tracks the grid width rather than a fixed column period and never
        // lands on the very edge.
        let entry_col = (width * (i + 1) / (spec.count + 1)).clamp(1, width - 2);
        let entry = entry_col; // row 0 → index = 0 * width + col = col

        // Per-river jitter via a child RNG seeded from RIVER_SEED + index.
        // Using a separate RNG keeps oxbow/tributary placement stable.
        let mut rrng = StdRng::seed_from_u64(RIVER_SEED ^ (i as u64 + 1));
        let drift_i = (spec.drift
            * (1.0 + rrng.random_range(-spec.drift_spread..spec.drift_spread)))
        .max(0.001);
        let bendiness_i = (spec.bendiness
            + rrng.random_range(-spec.bendiness_spread..spec.bendiness_spread))
        .clamp(0.0, 1.0);
        let penalty_i = lerp(PEN_HIGH as f32, PEN_LOW as f32, bendiness_i) as u32;

        // Drift the exit rightward by `drift_i * height` columns.
        let exit_col = ((entry_col as f32 + drift_i * height as f32).round() as isize)
            .clamp(0, width as isize - 1) as usize;
        let exit_idx = (height - 1) * width + exit_col;

        let cl = carve(grid, &cost, entry, exit_idx, Some((&spec.heading, penalty_i)));
        rasterize(grid, &cl, spec.radius, spec.core, 1.0);
        mains.push(cl);
    }

    // Oxbow pools: local cost minima not adjacent to any carved channel cell.
    let mut cands: Vec<usize> = (0..grid.len())
        .filter(|&i| {
            grid.water(i) == 0.0
                && [(-1, 0), (1, 0), (0, -1i32), (0, 1i32)]
                    .iter()
                    .all(|&(dx, dy)| {
                        grid.step(i, dx as isize, dy as isize)
                            .is_none_or(|n| grid.water(n) == 0.0)
                    })
                && [(-1, 0), (1, 0), (0, -1i32), (0, 1i32)]
                    .iter()
                    .all(|&(dx, dy)| {
                        grid.step(i, dx as isize, dy as isize)
                            .is_none_or(|n| cost[n] >= cost[i])
                    })
        })
        .collect();
    for _ in 0..spec.oxbow_count.min(cands.len()) {
        let ci = rng.random_range(0..cands.len());
        let center = cands.swap_remove(ci);
        rasterize(grid, &[center], spec.oxbow_radius, spec.core, spec.oxbow_depth);
    }

    // Lakes: larger standing-water bodies at the deepest non-channel basins.
    // Re-collect candidates so oxbow-painted cells are excluded.
    if spec.lake_count > 0 {
        let mut lake_cands: Vec<usize> = (0..grid.len())
            .filter(|&i| {
                grid.water(i) == 0.0
                    && [(-1, 0), (1, 0), (0, -1i32), (0, 1i32)]
                        .iter()
                        .all(|&(dx, dy)| {
                            grid.step(i, dx as isize, dy as isize)
                                .is_none_or(|n| grid.water(n) == 0.0)
                        })
                    && [(-1, 0), (1, 0), (0, -1i32), (0, 1i32)]
                        .iter()
                        .all(|&(dx, dy)| {
                            grid.step(i, dx as isize, dy as isize)
                                .is_none_or(|n| cost[n] >= cost[i])
                        })
            })
            .collect();
        // Pick the deepest basins (lowest cost) for deterministic placement.
        lake_cands.sort_by_key(|&i| cost[i]);
        for &center in lake_cands.iter().take(spec.lake_count) {
            rasterize(grid, &[center], spec.lake_radius, spec.lake_core, 1.0);
        }
    }

    // Tributaries: shallow feeders that branch off a main channel at a confluence.
    // Each feeder's goal is a cell sampled from a main centerline (by construction
    // it must join the main river), carved with plain least-cost (no heading bias).
    let mut tribs: Vec<Vec<usize>> = Vec::new();
    if !mains.is_empty() {
        let upper_half = height / 2;
        for _ in 0..spec.tributaries {
            let main_idx = rng.random_range(0..mains.len());
            let main_len = mains[main_idx].len();

            // Confluence from the interior: exclude first/last ~10% of the path.
            let skip = (main_len / 10).max(1);
            let lo = skip;
            let hi = main_len.saturating_sub(skip);
            if lo >= hi {
                continue;
            }
            let conf_pos = rng.random_range(lo..hi);
            let confluence = mains[main_idx][conf_pos]; // usize is Copy

            // Source on a side edge (left or right), upper portion of the map.
            let source_row = rng.random_range(0..upper_half.max(1));
            let source = if rng.random::<bool>() {
                source_row * width          // left edge, col 0
            } else {
                source_row * width + (width - 1) // right edge
            };

            let tcl = carve(grid, &cost, source, confluence, None);
            rasterize(grid, &tcl, spec.trib_radius, spec.core, spec.trib_depth);
            tribs.push(tcl);
        }
    }

    // Fords: periodic shallow crossing bands (horizontal, perpendicular to flow)
    // along EACH main centerline.
    for main_cl in &mains {
        for &ford_center in &ford_indices(main_cl, spec.ford_spacing) {
            for dx in -spec.radius..=spec.radius {
                if let Some(cell) = grid.step(ford_center, dx, 0) {
                    grid.set_water(cell, spec.ford_depth);
                    grid.set_ford(cell, true);
                }
            }
        }
    }

    compute_water_prox(grid, spec.water_reach);

    (mains, tribs)
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

/// Returns every `spacing`-th element of `centerline` by position (0, spacing, 2*spacing, …).
fn ford_indices(centerline: &[usize], spacing: usize) -> Vec<usize> {
    centerline.iter().copied().step_by(spacing).collect()
}

/// Multi-source BFS out from every water cell, converting ring-distance into a
/// grass carrying capacity: 0 in water, 1 right beside it, linearly down to 0 at
/// `reach` cells away.
fn compute_water_prox(grid: &mut Grid, reach: u32) {
    let len = grid.len();
    let mut dist = vec![u32::MAX; len];
    let mut queue = VecDeque::new();

    for (i, d) in dist.iter_mut().enumerate() {
        if grid.water(i) > 0.0 {
            *d = 0;
            queue.push_back(i);
        }
    }

    while let Some(index) = queue.pop_front() {
        let d = dist[index];
        for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
            if let Some(n) = grid.step(index, dx, dy) && dist[n] == u32::MAX {
                dist[n] = d + 1;
                queue.push_back(n);
            }
        }
    }

    for (i, &d) in dist.iter().enumerate() {
        let prox = match d {
            0 => 0.0,
            d => (1.0 - (d as f32 - 1.0) / reach as f32).max(0.0),
        };
        grid.set_water_prox(i, prox);
    }
}

pub struct RiverPlugin;

impl Plugin for RiverPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, generate_river);
    }
}

/// Build the white-noise cost field and smooth it `passes` times via `field::smooth`.
fn cost_field(grid: &Grid, rng: &mut StdRng, passes: usize) -> Vec<u32> {
    let white: Vec<f32> = (0..grid.len()).map(|_| rng.random::<f32>()).collect();
    let smoothed = field::smooth(&white, grid.width(), grid.height(), passes);
    smoothed.iter().map(|v| (v * 1000.0) as u32 + 1).collect()
}

/// Dijkstra on a grid graph. When `bias` is `Some((Heading::Down, penalty))`,
/// steps that go against the heading (upward: dy = -1) pay an extra `penalty`
/// on top of the cell's noise cost, biasing the path to descend. Sideways steps
/// are free; the lateral drift is encoded in the goal offset, not here.
fn carve(
    grid: &Grid,
    cost: &[u32],
    start: usize,
    goal: usize,
    bias: Option<(&Heading, u32)>,
) -> Vec<usize> {
    let len = grid.len();
    let mut dist = vec![u32::MAX; len];
    let mut prev = vec![usize::MAX; len];
    let mut heap = BinaryHeap::new();

    dist[start] = 0;
    heap.push(Reverse((0u32, start)));

    while let Some(Reverse((d, index))) = heap.pop() {
        if index == goal {
            break;
        }
        if d > dist[index] {
            continue;
        }
        for (dx, dy) in [(-1, 0), (1, 0), (0, -1i32), (0, 1i32)] {
            if let Some(n) = grid.step(index, dx as isize, dy as isize) {
                let dir_pen: u32 = match &bias {
                    // Against Heading::Down means going up (dy == -1).
                    Some((Heading::Down, penalty)) if dy == -1 => *penalty,
                    _ => 0,
                };
                let nd = d.saturating_add(cost[n]).saturating_add(dir_pen);
                if nd < dist[n] {
                    dist[n] = nd;
                    prev[n] = index;
                    heap.push(Reverse((nd, n)));
                }
            }
        }
    }

    // Reconstruct path from goal back to start; path[0] = goal, path.last() = start.
    let mut path = Vec::new();
    let mut cur = goal;
    while cur != usize::MAX {
        path.push(cur);
        if cur == start {
            break;
        }
        cur = prev[cur];
    }
    path
}

/// Stamp water values around each centerline cell. `max_depth` scales the level:
/// 1.0 gives a full-depth main channel; lower values produce shallow streams/pools.
/// Uses max-merge so overlapping channels keep the deeper value.
fn rasterize(grid: &mut Grid, centerline: &[usize], radius: isize, core: isize, max_depth: f32) {
    for &center in centerline {
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                if let Some(cell) = grid.step(center, dx, dy) {
                    let dist = ((dx * dx + dy * dy) as f32).sqrt();
                    let level = if dist <= core as f32 {
                        max_depth
                    } else {
                        (max_depth * (1.0 - dist / (radius as f32 + 1.0))).max(0.0)
                    };
                    if level > grid.water(cell) {
                        grid.set_water(cell, level);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::Grid;

    #[test]
    fn ford_indices_returns_every_nth_cell() {
        let cl: Vec<usize> = (10..110).collect(); // 100 elements: 10..109
        let fords = ford_indices(&cl, 25);
        assert_eq!(fords, vec![10, 35, 60, 85]);
    }

    #[test]
    fn ford_indices_empty_centerline() {
        assert!(ford_indices(&[], 10).is_empty());
    }

    #[test]
    fn ford_indices_spacing_larger_than_len() {
        let cl = vec![7usize, 8, 9];
        let fords = ford_indices(&cl, 10);
        assert_eq!(fords, vec![7]); // only position 0
    }

    #[test]
    fn generation_is_deterministic() {
        let spec = RiverSpec::default();
        let mut g1 = Grid::new(128, 96);
        let mut g2 = Grid::new(128, 96);
        generate_river_inner(&mut g1, &spec);
        generate_river_inner(&mut g2, &spec);
        let w1: Vec<f32> = (0..g1.len()).map(|i| g1.water(i)).collect();
        let w2: Vec<f32> = (0..g2.len()).map(|i| g2.water(i)).collect();
        assert_eq!(w1, w2, "same seed must produce identical water fields");
    }

    #[test]
    fn fords_carry_low_water() {
        let spec = RiverSpec::default();
        let mut g = Grid::new(128, 96);
        generate_river_inner(&mut g, &spec);
        for i in 0..g.len() {
            if g.is_ford(i) {
                assert!(
                    g.water(i) <= spec.ford_depth + f32::EPSILON,
                    "ford cell {i} has water {} > ford_depth {}",
                    g.water(i),
                    spec.ford_depth
                );
            }
        }
    }

    #[test]
    fn main_channel_has_deep_water() {
        let spec = RiverSpec::default();
        let mut g = Grid::new(128, 96);
        generate_river_inner(&mut g, &spec);
        let deep_cells = (0..g.len()).filter(|&i| g.water(i) > 0.9).count();
        assert!(deep_cells > 0, "expected deep main-channel core cells");
    }

    #[test]
    fn water_levels_span_expected_range() {
        let spec = RiverSpec::default();
        let mut g = Grid::new(128, 96);
        generate_river_inner(&mut g, &spec);
        let max_water = (0..g.len()).map(|i| g.water(i)).fold(0.0f32, f32::max);
        let has_shallow = (0..g.len()).any(|i| g.water(i) > 0.0 && g.water(i) < 0.5);
        assert!(max_water > 0.9, "expected deep main-channel water, got max={max_water}");
        assert!(has_shallow, "expected shallow tributary/ford cells");
    }

    /// Assert each main channel runs top→bottom with rightward drift.
    #[test]
    fn rivers_flow_top_to_bottom() {
        let spec = RiverSpec::default();
        let width = 128usize;
        let height = 96usize;
        let mut g = Grid::new(width, height);
        let (mains, _) = generate_river_inner(&mut g, &spec);

        assert_eq!(mains.len(), spec.count, "should carve {} main channels", spec.count);

        for (i, cl) in mains.iter().enumerate() {
            assert!(!cl.is_empty(), "main centerline {i} is empty");

            // carve() builds the path goal→start: cl[0] = exit (bottom), cl.last() = entry (top).
            let entry_idx = *cl.last().unwrap();
            let exit_idx = cl[0];

            let entry_row = entry_idx / width;
            let exit_row = exit_idx / width;

            assert_eq!(entry_row, 0, "main {i} entry must be row 0, got {entry_row}");
            assert_eq!(
                exit_row,
                height - 1,
                "main {i} exit must be row {}, got {exit_row}",
                height - 1
            );

            let entry_col = entry_idx % width;
            let exit_col = exit_idx % width;

            // Default drift = 0.25 * 96 = 24 columns rightward.
            assert!(
                exit_col > entry_col,
                "main {i} should drift rightward: entry_col={entry_col}, exit_col={exit_col}"
            );
        }
    }

    /// Assert each tributary's confluence cell (its Dijkstra goal) lies on a main
    /// channel centerline — the feeder reaches the main river by construction.
    /// We check centerline membership rather than water level because ford stamping
    /// unconditionally overwrites water to ford_depth on crossing cells, which can
    /// include a confluence that happens to fall under a ford band.
    #[test]
    fn tributaries_join_a_main_channel() {
        let spec = RiverSpec::default();
        let mut g = Grid::new(128, 96);
        let (mains, tribs) = generate_river_inner(&mut g, &spec);

        // We expect the default number of tributaries to have been carved.
        assert_eq!(
            tribs.len(),
            spec.tributaries,
            "expected {} tributaries",
            spec.tributaries
        );

        // Build a flat set of all main-centerline cells for O(1) lookup.
        let main_cells: std::collections::HashSet<usize> =
            mains.iter().flatten().copied().collect();

        for (i, tcl) in tribs.iter().enumerate() {
            assert!(!tcl.is_empty(), "tributary {i} centerline is empty");

            // carve() path[0] = goal = confluence cell (sampled from a main centerline).
            let confluence = tcl[0];
            assert!(
                main_cells.contains(&confluence),
                "tributary {i} confluence cell {confluence} is not on any main centerline"
            );
        }
    }

    /// Assert that per-river child RNGs produce different drift parameters,
    /// resulting in measurably different exit-column drift ratios across rivers.
    #[test]
    fn per_river_params_differ() {
        let spec = RiverSpec {
            drift_spread: 0.5,
            bendiness_spread: 0.5,
            ..RiverSpec::default()
        };
        let width = 128usize;
        let height = 96usize;
        let mut g = Grid::new(width, height);
        let (mains, _) = generate_river_inner(&mut g, &spec);

        assert!(mains.len() >= 2, "need at least 2 rivers to compare");

        let drifts: Vec<f32> = mains
            .iter()
            .map(|cl| {
                let entry_col = (*cl.last().unwrap() % width) as f32;
                let exit_col = (cl[0] % width) as f32;
                (exit_col - entry_col) / height as f32
            })
            .collect();

        let all_equal = drifts.windows(2).all(|w| (w[0] - w[1]).abs() < 0.01);
        assert!(
            !all_equal,
            "expected rivers to show per-river drift variation with spread=0.5, got: {:?}",
            drifts
        );
    }

    /// Assert that lakes add full-depth standing water beyond what channels alone produce.
    #[test]
    fn lakes_are_seeded() {
        let spec_with = RiverSpec::default(); // lake_count = 3
        let spec_none = RiverSpec { lake_count: 0, ..RiverSpec::default() };

        let mut g_with = Grid::new(128, 96);
        let mut g_none = Grid::new(128, 96);
        generate_river_inner(&mut g_with, &spec_with);
        generate_river_inner(&mut g_none, &spec_none);

        let deep_with = (0..g_with.len()).filter(|&i| g_with.water(i) >= 0.9).count();
        let deep_none = (0..g_none.len()).filter(|&i| g_none.water(i) >= 0.9).count();

        assert!(
            deep_with > deep_none,
            "expected lakes to add deep water cells: with_lakes={deep_with}, no_lakes={deep_none}"
        );
    }
}
