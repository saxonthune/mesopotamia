//! Two-pass lake generation: BIG lakes at interior peaks (farthest from any river or edge),
//! MINOR lakes blue-noise scattered and repelled by bigs. Metaball footprints so basins vary.

use std::collections::VecDeque;

use rand::Rng;
use rand::rngs::StdRng;
use rand::SeedableRng;

use crate::grid::Grid;

use super::spec::RiverSpec;

// Dedicated lake RNG so lake placement draws no entropy from the shared oxbow/tributary stream.
const LAKE_SALT: u64 = 0x1A4E;
// Per-pass salts prevent shape RNG collisions when big and minor lake indices match.
const BIG_SALT: u64 = 0xB16;
const MINOR_SALT: u64 = 0x5A11;

const NEIGHBORS: [(i32, i32); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];

// Like is_dry_basin but without the cost-local-minimum clause — spread beats siting in the deepest pit.
fn is_lake_site(grid: &Grid, i: usize) -> bool {
    grid.water(i) == 0.0
        && NEIGHBORS.iter().all(|&(dx, dy)| {
            grid.step(i, dx as isize, dy as isize)
                .is_none_or(|n| grid.water(n) == 0.0)
        })
}

// Uncapped BFS distance to water — no reach cap, so high values mark genuinely interior open space.
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

// Grid boundary treated as a shore: prevents basins seating so close they are clipped into a half-shape.
fn edge_dist(col: usize, row: usize, width: usize, height: usize) -> u32 {
    (col.min(row).min(width - 1 - col).min(height - 1 - row)) as u32
}

// Mitchell's best-candidate. `preseed` repellers let minor-pass centers avoid big-lake centers.
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

fn kernel(d2: f32, radius: f32) -> f32 {
    (1.0 - d2 / (radius * radius)).max(0.0)
}

fn basin_field(col: f32, row: f32, kernels: &[(f32, f32, f32)]) -> f32 {
    kernels
        .iter()
        .map(|&(cx, cy, r)| {
            let (dx, dy) = (col - cx, row - cy);
            kernel(dx * dx + dy * dy, r)
        })
        .sum()
}

fn lake_depth_at(field: f32, iso: f32) -> f32 {
    if field < iso {
        0.0
    } else {
        (field - iso).min(1.0)
    }
}

// Small offsets bulge the basin; medium offsets read as a peanut.
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

// Max-merge with existing water so deeper river water is never erased.
fn stamp_lakes(
    grid: &mut Grid,
    centers: &[usize],
    spec: &RiverSpec,
    base_radius: f32,
    lobes_max: usize,
    salt: u64,
) {
    let width = grid.width();
    let reach = (spec.lake_offset + base_radius * 1.2).ceil() as isize;

    for (li, &center) in centers.iter().enumerate() {
        let center_col = (center % width) as f32;
        let center_row = (center / width) as f32;
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

pub(super) fn generate_lakes(grid: &mut Grid, _cost: &[u32], spec: &RiverSpec) {
    if spec.big_lake_count == 0 && spec.minor_lake_count == 0 {
        return;
    }
    let width = grid.width();
    let height = grid.height();
    let len = grid.len();
    let mut rng = StdRng::seed_from_u64(spec.seed ^ LAKE_SALT);

    let sites: Vec<usize> = (0..len).filter(|&i| is_lake_site(grid, i)).collect();

    // `border[i]` = min(dist-to-water, dist-to-edge) so big lakes pool in interior pockets, not corners.
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

    // Edge clearance so the metaball footprint fits whole, never clipped into a swept shape.
    let big_reach = (spec.lake_offset + spec.big_lake_radius as f32 * 1.2).ceil() as u32;
    let minor_reach = (spec.lake_offset + spec.minor_lake_radius as f32 * 1.2).ceil() as u32;

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

    // Pass 2: minor lakes repelled by bigs.
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
        let beside = 3 * width + (mid - 1);
        assert_eq!(dist[beside], 1, "cell adjacent to water should be 1");
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
        assert_eq!(edge_dist(0, 0, 10, 8), 0);
        assert_eq!(edge_dist(1, 1, 10, 8), 1);
        assert_eq!(edge_dist(9, 7, 10, 8), 0, "the far corner is also on the edge");
        assert_eq!(edge_dist(5, 4, 10, 8), 3, "min(5, 4, 4, 3) = 3");
    }

    #[test]
    fn big_lakes_are_interior_and_clear_of_borders() {
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
        let kernels = [(20.0f32, 20.0f32, 5.0f32), (28.0f32, 20.0f32, 5.0f32)];
        let iso = 0.5f32;
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
        assert_eq!(lake_depth_at(5.0, iso), 1.0, "depth clamps at 1");
    }
}
