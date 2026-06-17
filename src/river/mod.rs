//! Procedural watershed generation. The pipeline composes the submodules: a
//! shared smoothed cost field and least-cost carve (`cost`), water stamped onto
//! the grid plus ford bands (`raster`), features layered on top (`features`:
//! oxbows, lakes, tributaries), and finally the water-proximity capacity field
//! (`prox`). Every knob lives in `RiverSpec` (`spec`); the whole water field is a
//! pure function of that spec plus `RIVER_SEED`.

mod cost;
mod features;
mod prox;
mod raster;
mod spec;

pub use spec::{Heading, RiverSpec};

use bevy::prelude::*;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use crate::grid::Grid;

use cost::{carve, cost_field, lerp};
use raster::{rasterize, stamp_fords};
use spec::{PEN_HIGH, PEN_LOW, RIVER_SEED};

pub struct RiverPlugin;

impl Plugin for RiverPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, generate_river);
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

    // Features layered on the main channels, in order: oxbows, then lakes (which
    // re-collect candidates so oxbow cells are excluded), then tributaries.
    features::place_oxbows(grid, &cost, &mut rng, spec);
    features::place_lakes(grid, &cost, spec);
    let tribs = features::carve_tributaries(grid, &cost, &mut rng, spec, &mains);

    stamp_fords(grid, &mains, spec.radius, spec.ford_spacing, spec.ford_depth);

    prox::compute_water_prox(grid, spec.water_reach);

    (mains, tribs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::Grid;

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
