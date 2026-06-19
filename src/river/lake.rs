//! Lakes as a real algorithm: blue-noise positions via Mitchell's best-candidate
//! so they spread rather than clump, and metaball footprints (summed radial
//! kernels cut at an iso-level) so each basin is filled-in but varied — round,
//! peanut, or oblong bulge — never a stamped circle. The summed field doubles as
//! depth: deepest at the kernels, shallow at the rim. Pure placement and shape
//! functions are tested below; `generate_lakes` wires them to the grid.

use rand::Rng;
use rand::rngs::StdRng;
use rand::SeedableRng;

use crate::grid::Grid;

use super::spec::RiverSpec;

/// Salt mixed into `spec.seed` for the dedicated lake RNG, so lake placement
/// draws no entropy from the shared oxbow/tributary stream — those stay
/// byte-identical regardless of lake knobs.
const LAKE_SALT: u64 = 0x1A4E;

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

/// Mitchell's best-candidate: pick `count` cells from `candidates` so they spread
/// with blue-noise spacing. The first is a random candidate; each subsequent pick
/// draws `samples` random candidates and keeps the one whose nearest already-placed
/// lake is farthest (Euclidean on `(col, row)`, `col = i % width`). Each pick is
/// thus repelled by the lakes already down. Returns the chosen center indices.
fn lake_positions(
    candidates: &[usize],
    count: usize,
    samples: usize,
    width: usize,
    rng: &mut StdRng,
) -> Vec<usize> {
    if candidates.is_empty() || count == 0 {
        return Vec::new();
    }
    let coords = |i: usize| ((i % width) as f32, (i / width) as f32);

    let mut chosen: Vec<usize> = Vec::with_capacity(count.min(candidates.len()));
    let first = candidates[rng.random_range(0..candidates.len())];
    chosen.push(first);

    while chosen.len() < count.min(candidates.len()) {
        let mut best = candidates[0];
        let mut best_dist = -1.0f32;
        for _ in 0..samples.max(1) {
            let cand = candidates[rng.random_range(0..candidates.len())];
            let (cx, cy) = coords(cand);
            let min_dist = chosen
                .iter()
                .map(|&p| {
                    let (px, py) = coords(p);
                    let (dx, dy) = (cx - px, cy - py);
                    dx * dx + dy * dy
                })
                .fold(f32::INFINITY, f32::min);
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

/// Place all lakes: pick spaced centers (best-candidate) and stamp a metaball
/// basin at each (summed kernels cut at `lake_iso`, the field doubling as depth).
/// Builds its own dedicated lake RNG from `spec.seed`, so it draws nothing from
/// the shared oxbow/tributary stream. Lakes max-merge with existing water, so
/// they never erase deeper river water and merge cleanly where they touch.
pub(super) fn generate_lakes(grid: &mut Grid, _cost: &[u32], spec: &RiverSpec) {
    if spec.lake_count == 0 {
        return;
    }
    let width = grid.width();
    let mut rng = StdRng::seed_from_u64(spec.seed ^ LAKE_SALT);

    let candidates: Vec<usize> = (0..grid.len()).filter(|&i| is_lake_site(grid, i)).collect();
    let centers = lake_positions(&candidates, spec.lake_count, spec.lake_samples, width, &mut rng);

    let base_radius = spec.lake_radius as f32;
    // Bounding-box reach: a kernel center can sit `lake_offset` cells out, and its
    // jittered radius reaches up to 1.2× the base, so paint that far around center.
    let reach = (spec.lake_offset + base_radius * 1.2).ceil() as isize;

    for (li, &center) in centers.iter().enumerate() {
        let center_col = (center % width) as f32;
        let center_row = (center / width) as f32;
        // Per-lake child RNG so each lake's shape is independently pinnable.
        let mut child = StdRng::seed_from_u64(spec.seed ^ LAKE_SALT ^ ((li as u64 + 1) << 24));
        let kernels = lake_kernels(
            center_col,
            center_row,
            base_radius,
            spec.lake_offset,
            spec.lake_lobes_max,
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
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lake_positions_are_spaced() {
        // A dense grid of candidate cells on a 64-wide field.
        let width = 64usize;
        let height = 64usize;
        let candidates: Vec<usize> = (0..width * height).collect();
        let mut rng = StdRng::seed_from_u64(42);
        let count = 8;
        let positions = lake_positions(&candidates, count, 16, width, &mut rng);

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
