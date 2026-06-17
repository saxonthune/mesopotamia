//! Features layered on top of the main channels — oxbow pools, lakes, and
//! tributaries. Each composes the shared cost field with `carve` and `rasterize`;
//! none authors a main channel itself.

use rand::Rng;
use rand::rngs::StdRng;

use crate::grid::Grid;

use super::cost::carve;
use super::raster::rasterize;
use super::spec::RiverSpec;

const NEIGHBORS: [(i32, i32); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];

/// A cell that holds no water and whose 4-neighbours hold none either — i.e. not
/// adjacent to any carved channel — and that is a local minimum of the cost field.
fn is_dry_basin(grid: &Grid, cost: &[u32], i: usize) -> bool {
    grid.water(i) == 0.0
        && NEIGHBORS.iter().all(|&(dx, dy)| {
            grid.step(i, dx as isize, dy as isize)
                .is_none_or(|n| grid.water(n) == 0.0)
        })
        && NEIGHBORS.iter().all(|&(dx, dy)| {
            grid.step(i, dx as isize, dy as isize)
                .is_none_or(|n| cost[n] >= cost[i])
        })
}

/// Oxbow pools: small pools dropped at randomly chosen dry cost-basins.
pub(super) fn place_oxbows(grid: &mut Grid, cost: &[u32], rng: &mut StdRng, spec: &RiverSpec) {
    let mut cands: Vec<usize> = (0..grid.len())
        .filter(|&i| is_dry_basin(grid, cost, i))
        .collect();
    for _ in 0..spec.oxbow_count.min(cands.len()) {
        let ci = rng.random_range(0..cands.len());
        let center = cands.swap_remove(ci);
        rasterize(grid, &[center], spec.oxbow_radius, spec.core, spec.oxbow_depth);
    }
}

/// Lakes: larger full-depth standing water at the deepest dry basins. Re-collects
/// candidates so oxbow-painted cells are excluded, and picks the lowest-cost
/// (deepest) basins for deterministic, intentional placement.
pub(super) fn place_lakes(grid: &mut Grid, cost: &[u32], spec: &RiverSpec) {
    if spec.lake_count == 0 {
        return;
    }
    let mut lake_cands: Vec<usize> = (0..grid.len())
        .filter(|&i| is_dry_basin(grid, cost, i))
        .collect();
    lake_cands.sort_by_key(|&i| cost[i]);
    for &center in lake_cands.iter().take(spec.lake_count) {
        rasterize(grid, &[center], spec.lake_radius, spec.lake_core, 1.0);
    }
}

/// Tributaries: shallow feeders that branch off a main channel at a confluence.
/// Each feeder's goal is a cell sampled from a main centerline (by construction
/// it must join the main river), carved with plain least-cost (no heading bias).
pub(super) fn carve_tributaries(
    grid: &mut Grid,
    cost: &[u32],
    rng: &mut StdRng,
    spec: &RiverSpec,
    mains: &[Vec<usize>],
) -> Vec<Vec<usize>> {
    let mut tribs: Vec<Vec<usize>> = Vec::new();
    if mains.is_empty() {
        return tribs;
    }
    let width = grid.width();
    let height = grid.height();
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
            source_row * width // left edge, col 0
        } else {
            source_row * width + (width - 1) // right edge
        };

        let tcl = carve(grid, cost, source, confluence, None);
        rasterize(grid, &tcl, spec.trib_radius, spec.core, spec.trib_depth);
        tribs.push(tcl);
    }
    tribs
}
