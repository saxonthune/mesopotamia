//! Features layered on top of the main channels — oxbow pools and tributaries.
//! Each composes the shared cost field with `carve` and `rasterize`; none authors
//! a main channel itself. Lakes are their own algorithm and live in `lake`.

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

/// Tributaries: short feeders spaced along each main channel. For each main,
/// steps every `trib_spacing` positions along the interior (skipping first/last
/// ~10%), picks a source offset ~`trib_length` cells to one lateral side and
/// slightly upstream, and carves a short feeder to that confluence point.
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

    for main in mains {
        let main_len = main.len();
        let skip = (main_len / 10).max(1);
        let interior = &main[skip..main_len.saturating_sub(skip)];
        if interior.is_empty() {
            continue;
        }

        for confluence_idx in interior.iter().copied().step_by(spec.trib_spacing.max(1)) {
            let conf_col = (confluence_idx % width) as isize;
            let conf_row = (confluence_idx / width) as isize;

            // Offset the source slightly upstream (smaller row) and laterally.
            let half = spec.trib_length / 2;
            let src_row = (conf_row - half).clamp(0, height as isize - 1);

            // Pick left or right side with the seeded rng; try the other if off-grid.
            let left_col = conf_col - spec.trib_length;
            let right_col = conf_col + spec.trib_length;
            let prefer_left: bool = rng.random();
            let src_col = if prefer_left {
                if left_col >= 0 { left_col } else if right_col < width as isize { right_col } else { continue }
            } else {
                if right_col < width as isize { right_col } else if left_col >= 0 { left_col } else { continue }
            };

            let source = src_row as usize * width + src_col as usize;

            // Skip if the candidate source already sits in water.
            if grid.water(source) > 0.0 {
                continue;
            }

            let tcl = carve(grid, cost, source, confluence_idx, None);
            rasterize(grid, &tcl, spec.trib_radius, spec.core, spec.trib_depth);
            tribs.push(tcl);
        }
    }
    tribs
}
