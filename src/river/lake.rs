//! Lakes as a real algorithm, in two passes: a few BIG lakes pooled at the
//! interior peaks of the open spaces the rivers leave (cells farthest from any
//! border — a river OR the grid edge), then a couple of MINOR lakes scattered and
//! repelled by the big ones. Both passes seat blue-noise positions via Mitchell's
//! best-candidate so lakes spread rather than clump, and stamp metaball footprints
//! (summed radial kernels cut at an iso-level) so each basin is filled-in but
//! varied — round, peanut, or oblong bulge — never a stamped circle. The summed
//! field doubles as depth: deepest at the kernels, shallow at the rim. Pure
//! placement, distance, and shape functions are tested below; `generate_lakes`
//! wires them to the grid.

use std::collections::VecDeque;

use rand::Rng;
use rand::rngs::StdRng;
use rand::SeedableRng;

use crate::grid::Grid;

use super::spec::RiverSpec;

/// Salt mixed into `spec.seed` for the dedicated lake RNG, so lake placement
/// draws no entropy from the shared oxbow/tributary stream — those stay
/// byte-identical regardless of lake knobs.
const LAKE_SALT: u64 = 0x1A4E;

/// Per-pass child-seed salts, so the two passes' per-lake shape RNGs never
/// collide even when a big lake and a minor lake share a lake index.
const BIG_SALT: u64 = 0xB16;
const MINOR_SALT: u64 = 0x5A11;

const NEIGHBORS: [(i32, i32); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];

/// A cell eligible to seat a lake: holds no water and neither do its 4-neighbours
/// (so lakes stay clear of the carved channels). A relaxed `is_dry_basin` — the
/// cost-local-minimum clause is dropped, because best-candidate spacing should
/// not fight a sparse basin set; spread is prioritised over sitting in the
/// deepest pit.
fn is_lake_site(grid: &Grid, i: usize) -> bool {
    grid.water(i) == 0.0
        && NEIGHBORS.iter().all(|&(dx, dy)| {
            grid.step(i, dx as isize, dy as isize)
                .is_none_or(|n| grid.water(n) == 0.0)
        })
}

/// Uncapped graph distance to the nearest water cell, for every cell, via a
/// multi-source BFS seeded from all `water(i) > 0.0` cells (4-connectivity).
/// Mirrors `prox::compute_water_prox` but WITHOUT the reach cap: the raw ring
/// distance survives, so high values mark cells deep in the open space between
/// rivers. A cell adjacent to water is 1; a cell with no water anywhere stays
/// `u32::MAX`. The "center of the open spaces" signal pass 1 keys off.
fn dist_to_water(grid: &Grid) -> Vec<u32> {
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
        for &(dx, dy) in NEIGHBORS.iter() {
            if let Some(n) = grid.step(index, dx as isize, dy as isize)
                && dist[n] == u32::MAX
            {
                dist[n] = d + 1;
                queue.push_back(n);
            }
        }
    }
    dist
}

/// Cells from `(col, row)` to the nearest grid edge — the min of the four edge
/// gaps. The grid boundary treated as a shore: lakes are spaced from it just as
/// they are from rivers, so a basin never seats close enough to be clipped into a
/// swept half-shape.
fn edge_dist(col: usize, row: usize, width: usize, height: usize) -> u32 {
    (col.min(row).min(width - 1 - col).min(height - 1 - row)) as u32
}

/// Mitchell's best-candidate: pick `count` cells from `candidates` so they spread
/// with blue-noise spacing, repelled by both each other AND the `preseed`
/// repellers (already-placed centers from an earlier pass). Each pick draws
/// `samples` random candidates and keeps the one whose nearest already-placed-or-
/// preseeded center is farthest (Euclidean on `(col, row)`, `col = i % width`).
/// With an empty `preseed` the first pick is a single random candidate (the
/// original blue-noise seed); with a non-empty `preseed` even the first pick is
/// repelled, so a minor lake never lands on a big lake. Returns only the newly
/// chosen center indices (not the preseed).
fn lake_positions(
    candidates: &[usize],
    count: usize,
    samples: usize,
    width: usize,
    preseed: &[usize],
    rng: &mut StdRng,
) -> Vec<usize> {
    if candidates.is_empty() || count == 0 {
        return Vec::new();
    }
    let coords = |i: usize| ((i % width) as f32, (i / width) as f32);

    // Nearest squared distance from `cand` to any repeller (preseed + chosen).
    let repels = |chosen: &[usize], cand: usize| -> f32 {
        let (cx, cy) = coords(cand);
        preseed
            .iter()
            .chain(chosen.iter())
            .map(|&p| {
                let (px, py) = coords(p);
                let (dx, dy) = (cx - px, cy - py);
                dx * dx + dy * dy
            })
            .fold(f32::INFINITY, f32::min)
    };

    let mut chosen: Vec<usize> = Vec::with_capacity(count.min(candidates.len()));
    if preseed.is_empty() {
        // No repellers yet: seed with a single random candidate.
        chosen.push(candidates[rng.random_range(0..candidates.len())]);
    }

    while chosen.len() < count.min(candidates.len()) {
        let mut best = candidates[0];
        let mut best_dist = -1.0f32;
        for _ in 0..samples.max(1) {
            let cand = candidates[rng.random_range(0..candidates.len())];
            let min_dist = repels(&chosen, cand);
            if min_dist > best_dist {
                best_dist = min_dist;
                best = cand;
            }
        }
        chosen.push(best);
    }
    chosen
}

/// Radial kernel falloff: smooth inside `radius`, zero outside. Takes squared
/// distance so no sqrt is needed. `(1 - d2/radius²).max(0)`.
fn kernel(d2: f32, radius: f32) -> f32 {
    (1.0 - d2 / (radius * radius)).max(0.0)
}

/// Summed metaball field at `(col, row)` from a kernel set, each kernel
/// `(center_col, center_row, radius)`. The sum of radial falloffs — one kernel
/// reads round, two offset kernels read peanut/bulge.
fn basin_field(col: f32, row: f32, kernels: &[(f32, f32, f32)]) -> f32 {
    kernels
        .iter()
        .map(|&(cx, cy, r)| {
            let (dx, dy) = (col - cx, row - cy);
            kernel(dx * dx + dy * dy, r)
        })
        .sum()
}

/// Turn a metaball field value into a lake depth: `0` below the iso threshold,
/// else a positive depth rising with the field and clamped to `1`. The field IS
/// the depth — deepest at the kernels, shallow at the rim.
fn lake_depth_at(field: f32, iso: f32) -> f32 {
    if field < iso {
        0.0
    } else {
        (field - iso).min(1.0)
    }
}

/// Derive a metaball kernel set for one lake from a per-lake child seed. A lobe
/// count biased toward 1–2 (range `1..=lobes_max`): the first kernel sits at the
/// center with `base_radius`; each extra kernel is offset up to `offset` cells in
/// a random direction with a jittered radius. Small offsets bulge, medium ones
/// read as a peanut.
fn lake_kernels(
    center_col: f32,
    center_row: f32,
    base_radius: f32,
    offset: f32,
    lobes_max: usize,
    rng: &mut StdRng,
) -> Vec<(f32, f32, f32)> {
    // Bias low: average two draws so the count leans toward 1–2.
    let hi = lobes_max.max(1);
    let a = rng.random_range(1..=hi);
    let b = rng.random_range(1..=hi);
    let lobes = ((a + b) / 2).clamp(1, hi);

    let mut kernels = Vec::with_capacity(lobes);
    kernels.push((center_col, center_row, base_radius));
    for _ in 1..lobes {
        let angle = rng.random_range(0.0..std::f32::consts::TAU);
        let dist = rng.random_range(0.0..offset);
        let kx = center_col + angle.cos() * dist;
        let ky = center_row + angle.sin() * dist;
        let kr = base_radius * rng.random_range(0.7..1.2);
        kernels.push((kx, ky, kr));
    }
    kernels
}

/// Stamp one set of lake centers onto the grid: a metaball basin at each (summed
/// kernels cut at `lake_iso`, the field doubling as depth), max-merging with
/// existing water so deeper river water is never erased. `base_radius` and
/// `lobes_max` are the per-pass shape knobs; `salt` separates each pass's per-lake
/// child seeds so their shapes are independent and the world stays a pure function
/// of the spec.
fn stamp_lakes(
    grid: &mut Grid,
    centers: &[usize],
    spec: &RiverSpec,
    base_radius: f32,
    lobes_max: usize,
    salt: u64,
) {
    let width = grid.width();
    // Bounding-box reach: a kernel center can sit `lake_offset` cells out, and its
    // jittered radius reaches up to 1.2× the base, so paint that far around center.
    let reach = (spec.lake_offset + base_radius * 1.2).ceil() as isize;

    for (li, &center) in centers.iter().enumerate() {
        let center_col = (center % width) as f32;
        let center_row = (center / width) as f32;
        // Per-lake child RNG so each lake's shape is independently pinnable.
        let mut child =
            StdRng::seed_from_u64(spec.seed ^ LAKE_SALT ^ salt ^ ((li as u64 + 1) << 24));
        let kernels = lake_kernels(
            center_col,
            center_row,
            base_radius,
            spec.lake_offset,
            lobes_max,
            &mut child,
        );

        for dy in -reach..=reach {
            for dx in -reach..=reach {
                if let Some(cell) = grid.step(center, dx, dy) {
                    let (col, row) = grid.col_row(cell);
                    let field = basin_field(col as f32, row as f32, &kernels);
                    let depth = lake_depth_at(field, spec.lake_iso);
                    if depth > 0.0 {
                        let existing = grid.water(cell);
                        grid.set_water(cell, existing.max(depth));
                        // Tag the cell as lake water so the proximity field gives it
                        // a tighter bank than a river channel.
                        grid.set_lake(cell, true);
                    }
                }
            }
        }
    }
}

/// Place all lakes in two passes. Pass 1: a few BIG lakes at the interior peaks —
/// best-candidate over the lake sites whose distance-to-water clears
/// `big_lake_dist_frac * max_distance`, stamped at `big_lake_radius`. Pass 2: a
/// couple of MINOR scattered lakes — best-candidate over all lake sites, with the
/// big-lake centers pre-seeded as repellers so minors avoid them, stamped at the
/// smaller `minor_lake_radius` with a low lobe cap. Both passes draw from one
/// dedicated lake RNG (`spec.seed ^ LAKE_SALT`), pass 1 fully then pass 2, so lake
/// placement draws no entropy from the shared oxbow/tributary stream and stays a
/// pure function of the spec. Lakes max-merge with existing water.
pub(super) fn generate_lakes(grid: &mut Grid, _cost: &[u32], spec: &RiverSpec) {
    if spec.big_lake_count == 0 && spec.minor_lake_count == 0 {
        return;
    }
    let width = grid.width();
    let height = grid.height();
    let len = grid.len();
    let mut rng = StdRng::seed_from_u64(spec.seed ^ LAKE_SALT);

    let sites: Vec<usize> = (0..len).filter(|&i| is_lake_site(grid, i)).collect();

    // Distance to the nearest border, materialized so the filters below carry no
    // borrow of the grid (`stamp_lakes` needs it mutably). `border[i]` is the min
    // of distance-to-river and distance-to-grid-edge: the grid boundary is treated
    // as just another shore, so big lakes pool in genuinely enclosed pockets rather
    // than in the corners (far from rivers but on the edge, where the basin clips
    // into the swept half-shape). `edges[i]` is kept separately to enforce a hard
    // edge clearance per pass so no footprint is ever cut by the bounds.
    let dist = dist_to_water(grid);
    let edges: Vec<u32> = (0..len)
        .map(|i| {
            let (col, row) = grid.col_row(i);
            edge_dist(col, row, width, height)
        })
        .collect();
    let border: Vec<u32> = (0..len).map(|i| dist[i].min(edges[i])).collect();

    let max_dist = sites.iter().map(|&i| border[i]).max().unwrap_or(0);
    let threshold = (spec.big_lake_dist_frac * max_dist as f32).ceil() as u32;

    // Basin reach per pass: a center must clear the grid edge by at least this many
    // cells so its metaball footprint fits whole, never clipped into a swept shape.
    let big_reach = (spec.lake_offset + spec.big_lake_radius as f32 * 1.2).ceil() as u32;
    let minor_reach = (spec.lake_offset + spec.minor_lake_radius as f32 * 1.2).ceil() as u32;

    // Pass 1 — big lakes at the interior peaks, clear of both rivers and the edge.
    let interior: Vec<usize> = sites
        .iter()
        .copied()
        .filter(|&i| border[i] >= threshold && edges[i] >= big_reach)
        .collect();
    let big_centers = lake_positions(
        &interior,
        spec.big_lake_count,
        spec.lake_samples,
        width,
        &[],
        &mut rng,
    );
    stamp_lakes(
        grid,
        &big_centers,
        spec,
        spec.big_lake_radius as f32,
        spec.lake_lobes_max,
        BIG_SALT,
    );

    // Pass 2 — minor scattered lakes, also held off the grid edge, repelled by bigs.
    let minor_sites: Vec<usize> =
        sites.iter().copied().filter(|&i| edges[i] >= minor_reach).collect();
    let minor_centers = lake_positions(
        &minor_sites,
        spec.minor_lake_count,
        spec.lake_samples,
        width,
        &big_centers,
        &mut rng,
    );
    let minor_lobes = 2.min(spec.lake_lobes_max);
    stamp_lakes(
        grid,
        &minor_centers,
        spec,
        spec.minor_lake_radius as f32,
        minor_lobes,
        MINOR_SALT,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a grid with a single column of water down the middle, so the
    /// distance-to-water field has a clean known structure for the BFS tests.
    fn grid_with_center_water(width: usize, height: usize) -> Grid {
        let mut g = Grid::new(width, height);
        let mid = width / 2;
        for row in 0..height {
            g.set_water(row * width + mid, 1.0);
        }
        g
    }

    #[test]
    fn dist_to_water_adjacency_is_one() {
        let width = 16usize;
        let height = 8usize;
        let g = grid_with_center_water(width, height);
        let dist = dist_to_water(&g);
        let mid = width / 2;
        // A cell directly beside the water column is distance 1.
        let beside = 3 * width + (mid - 1);
        assert_eq!(dist[beside], 1, "cell adjacent to water should be 1");
        // The water cell itself is 0.
        assert_eq!(dist[3 * width + mid], 0, "water cell is distance 0");
    }

    #[test]
    fn dist_to_water_interior_is_larger() {
        let width = 16usize;
        let height = 8usize;
        let g = grid_with_center_water(width, height);
        let dist = dist_to_water(&g);
        let mid = width / 2;
        let beside = 3 * width + (mid - 1);
        // A cell at the far edge sits deeper in the open space.
        let far = 3 * width; // column 0
        assert!(
            dist[far] > dist[beside],
            "interior cell {} (d={}) should exceed adjacency {} (d={})",
            far, dist[far], beside, dist[beside]
        );
        assert_eq!(dist[far], mid as u32, "column 0 is `mid` steps from center water");
    }

    #[test]
    fn dist_to_water_is_deterministic() {
        let g = grid_with_center_water(16, 8);
        assert_eq!(dist_to_water(&g), dist_to_water(&g), "BFS must be deterministic");
    }

    #[test]
    fn lake_positions_are_spaced() {
        // A dense grid of candidate cells on a 64-wide field.
        let width = 64usize;
        let height = 64usize;
        let candidates: Vec<usize> = (0..width * height).collect();
        let mut rng = StdRng::seed_from_u64(42);
        let count = 8;
        let positions = lake_positions(&candidates, count, 16, width, &[], &mut rng);

        assert_eq!(positions.len(), count, "should return `count` positions");

        // Blue-noise property: every pair is at least some floor apart. With 8
        // points over 64×64 a naive grid spacing is ~22 cells; demand a modest
        // floor that random placement would routinely violate.
        let coords = |i: usize| ((i % width) as f32, (i / width) as f32);
        let mut min_pair = f32::INFINITY;
        for a in 0..positions.len() {
            for b in (a + 1)..positions.len() {
                let (ax, ay) = coords(positions[a]);
                let (bx, by) = coords(positions[b]);
                let d = ((ax - bx).powi(2) + (ay - by).powi(2)).sqrt();
                min_pair = min_pair.min(d);
            }
        }
        assert!(
            min_pair > 8.0,
            "lakes should be blue-noise spaced; min pairwise distance was {min_pair}"
        );
    }

    #[test]
    fn lake_positions_avoid_preseed() {
        // Preseed a repeller in one corner; chosen centers should avoid it.
        let width = 64usize;
        let height = 64usize;
        let candidates: Vec<usize> = (0..width * height).collect();
        let preseed = vec![0usize]; // top-left corner
        let mut rng = StdRng::seed_from_u64(7);
        let positions = lake_positions(&candidates, 4, 32, width, &preseed, &mut rng);
        let coords = |i: usize| ((i % width) as f32, (i / width) as f32);
        for &p in &positions {
            let (px, py) = coords(p);
            let d = (px * px + py * py).sqrt();
            assert!(
                d > 8.0,
                "minor lake at ({px},{py}) landed on the preseeded repeller corner (d={d})"
            );
        }
    }

    #[test]
    fn edge_dist_measures_nearest_grid_edge() {
        // 10×8 grid: corners are 0, one cell in is 1, an interior cell is the min
        // of its four edge gaps.
        assert_eq!(edge_dist(0, 0, 10, 8), 0);
        assert_eq!(edge_dist(1, 1, 10, 8), 1);
        assert_eq!(edge_dist(9, 7, 10, 8), 0, "the far corner is also on the edge");
        assert_eq!(edge_dist(5, 4, 10, 8), 3, "min(5, 4, 4, 3) = 3");
    }

    #[test]
    fn big_lakes_are_interior_and_clear_of_borders() {
        // Derive the pass-1 placement on a real pre-lake water field (lakes
        // disabled), exactly as `generate_lakes` does, then assert the big lakes
        // clear the border (river-or-edge) threshold AND keep their basin reach off
        // the grid edge so nothing is clipped into a swept shape.
        use crate::river::{generate_water, RiverSpec};
        let spec = RiverSpec::default();
        let no_lake = RiverSpec { big_lake_count: 0, minor_lake_count: 0, ..RiverSpec::default() };
        let mut g0 = Grid::new(128, 96);
        generate_water(&mut g0, &no_lake);

        let (width, height) = (g0.width(), g0.height());
        let len = g0.len();
        let sites0: Vec<usize> = (0..len).filter(|&i| is_lake_site(&g0, i)).collect();
        let dist = dist_to_water(&g0);
        let edges: Vec<u32> = (0..len)
            .map(|i| {
                let (c, r) = g0.col_row(i);
                edge_dist(c, r, width, height)
            })
            .collect();
        let border: Vec<u32> = (0..len).map(|i| dist[i].min(edges[i])).collect();
        let max_dist = sites0.iter().map(|&i| border[i]).max().unwrap_or(0);
        let threshold = (spec.big_lake_dist_frac * max_dist as f32).ceil() as u32;
        let big_reach = (spec.lake_offset + spec.big_lake_radius as f32 * 1.2).ceil() as u32;

        let mut rng = StdRng::seed_from_u64(spec.seed ^ LAKE_SALT);
        let interior: Vec<usize> = sites0
            .iter()
            .copied()
            .filter(|&i| border[i] >= threshold && edges[i] >= big_reach)
            .collect();
        let big = lake_positions(&interior, spec.big_lake_count, spec.lake_samples, width, &[], &mut rng);

        assert!(!big.is_empty(), "expected big-lake centers");
        for &c in &big {
            assert!(
                border[c] >= threshold,
                "big lake at {c} (border={}) below interior threshold {threshold}",
                border[c]
            );
            assert!(
                edges[c] >= big_reach,
                "big lake at {c} (edge={}) too close to the grid border (need {big_reach})",
                edges[c]
            );
        }
    }

    #[test]
    fn basin_single_kernel_is_round() {
        // One centered kernel: the field at equal radii along x and y must match.
        let kernels = [(10.0f32, 10.0f32, 5.0f32)];
        for d in [1.0f32, 2.0, 3.0, 4.0] {
            let along_x = basin_field(10.0 + d, 10.0, &kernels);
            let along_y = basin_field(10.0, 10.0 + d, &kernels);
            assert!(
                (along_x - along_y).abs() < 1e-5,
                "single kernel should be symmetric in x and y at d={d}: {along_x} vs {along_y}"
            );
        }
    }

    #[test]
    fn basin_offset_kernels_are_asymmetric() {
        // Two kernels offset along x. The footprint (cells above the iso) must
        // reach measurably farther along x than along y.
        let kernels = [(20.0f32, 20.0f32, 5.0f32), (28.0f32, 20.0f32, 5.0f32)];
        let iso = 0.5f32;
        // Midpoint of the two centers.
        let (mx, my) = (24.0f32, 20.0f32);
        let mut reach_x = 0.0f32;
        let mut reach_y = 0.0f32;
        for d in 0..40 {
            let d = d as f32 * 0.5;
            if basin_field(mx + d, my, &kernels) >= iso {
                reach_x = reach_x.max(d);
            }
            if basin_field(mx, my + d, &kernels) >= iso {
                reach_y = reach_y.max(d);
            }
        }
        assert!(
            reach_x > reach_y + 1.0,
            "offset-along-x kernels should be longer in x: reach_x={reach_x}, reach_y={reach_y}"
        );
    }

    #[test]
    fn lake_depth_below_iso_is_zero() {
        assert_eq!(lake_depth_at(0.3, 0.5), 0.0, "field below iso yields no water");
    }

    #[test]
    fn lake_depth_rises_above_iso_and_clamps() {
        let iso = 0.5f32;
        let shallow = lake_depth_at(0.6, iso);
        let deeper = lake_depth_at(0.9, iso);
        assert!(shallow > 0.0, "field above iso yields positive depth");
        assert!(deeper > shallow, "depth rises with the field");
        // Field far above iso clamps to 1.
        assert_eq!(lake_depth_at(5.0, iso), 1.0, "depth clamps at 1");
    }
}
