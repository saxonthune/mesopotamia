//! The invisible continental-divide classification: a Voronoi assignment of every
//! cell to its nearest main river, recording which side of that river the cell
//! falls on. A multi-source BFS out from every main-centerline cell propagates the
//! source's column outward (nearest source wins), so each cell learns the column of
//! its nearest river. A cell strictly east (greater column) of that river is tagged
//! `east_of_river`. The watershed midway between adjacent rivers is just where the
//! nearest-river assignment flips — it is never drawn, only where the tag changes.

use std::collections::VecDeque;

use crate::grid::Grid;

/// Strictly-east comparison extracted as a pure function: the cell at `cell_col`
/// is east of the river whose nearest-centerline column is `river_col`.
fn is_east(cell_col: usize, river_col: usize) -> bool {
    cell_col > river_col
}

/// Classify every cell by which side of its nearest main river it falls on.
/// Multi-source BFS (4-connectivity) out from all main-centerline cells, carrying
/// the source cell's column; the first (nearest) source to reach a cell wins, so
/// each cell ends up with the column of its nearest river. Cells with no reachable
/// river (empty `mains`) stay `false` (west).
pub(super) fn seed_river_side(grid: &mut Grid, mains: &[Vec<usize>]) {
    let len = grid.len();
    // `nearest_col[i]` is the column of the nearest main-centerline cell; `usize::MAX`
    // marks unvisited. BFS by ring order means the first writer is the nearest source.
    let mut nearest_col = vec![usize::MAX; len];
    // BFS ring distance to the nearest river source — 0 at a centerline cell,
    // rising outward. The same wavefront that carries `nearest_col` measures it.
    let mut dist = vec![0u32; len];
    let mut queue = VecDeque::new();

    for cell in mains.iter().flatten().copied() {
        if nearest_col[cell] == usize::MAX {
            let (col, _) = grid.col_row(cell);
            nearest_col[cell] = col;
            queue.push_back(cell);
        }
    }

    while let Some(index) = queue.pop_front() {
        let src_col = nearest_col[index];
        let next_dist = dist[index] + 1;
        for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
            if let Some(n) = grid.step(index, dx, dy) && nearest_col[n] == usize::MAX {
                nearest_col[n] = src_col;
                dist[n] = next_dist;
                queue.push_back(n);
            }
        }
    }

    // Normalize distance by the global max so the farthest interfluve reads as 1
    // (the divide) and a river as 0. Guard against an empty wavefront (no mains).
    let max_dist = dist.iter().copied().max().unwrap_or(0).max(1) as f32;

    for i in 0..len {
        let (east, river_dist) = match nearest_col[i] {
            usize::MAX => (false, 0.0), // no reachable river → west, zero distance
            river_col => {
                let (cell_col, _) = grid.col_row(i);
                (is_east(cell_col, river_col), dist[i] as f32 / max_dist)
            }
        };
        grid.set_east_of_river(i, east);
        grid.set_river_dist(i, river_dist);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::Grid;

    /// A single straight vertical river at column `c` splits the grid: every cell
    /// east of it is `true`, every cell west of it is `false`.
    #[test]
    fn east_west_split_around_a_vertical_river() {
        let (width, height) = (16usize, 12usize);
        let c = 7usize;
        let mut grid = Grid::new(width, height);

        // One "main" centerline: a straight vertical line at column c.
        let main: Vec<usize> = (0..height).map(|row| row * width + c).collect();
        seed_river_side(&mut grid, &[main]);

        for row in 0..height {
            for col in 0..width {
                let i = row * width + col;
                let east = grid.east_of_river(i);
                if col > c {
                    assert!(east, "cell ({col},{row}) east of river col {c} should be east");
                } else if col < c {
                    assert!(!east, "cell ({col},{row}) west of river col {c} should be west");
                } else {
                    // On the river itself: not strictly east.
                    assert!(!east, "cell on river col {c} is not strictly east");
                }
            }
        }
    }

    /// No mains → no source reaches any cell → every cell stays west (false).
    #[test]
    fn no_mains_leaves_all_west() {
        let mut grid = Grid::new(10, 8);
        seed_river_side(&mut grid, &[]);
        for i in 0..grid.len() {
            assert!(!grid.east_of_river(i), "with no mains every cell must be west");
        }
    }

    /// The pass is deterministic and total: the same inputs classify the same cells
    /// the same way on a re-run.
    #[test]
    fn classification_is_deterministic() {
        let (width, height) = (12usize, 10usize);
        let main: Vec<usize> = (0..height).map(|row| row * width + 5).collect();

        let mut g1 = Grid::new(width, height);
        let mut g2 = Grid::new(width, height);
        seed_river_side(&mut g1, &[main.clone()]);
        seed_river_side(&mut g2, &[main]);

        let s1: Vec<bool> = (0..g1.len()).map(|i| g1.east_of_river(i)).collect();
        let s2: Vec<bool> = (0..g2.len()).map(|i| g2.east_of_river(i)).collect();
        assert_eq!(s1, s2, "same river must classify the same cells");
    }

    /// River-distance is 0 on the centerline and grows monotonically with column
    /// distance from a single straight vertical river; the farthest cell reads 1.
    #[test]
    fn river_dist_rises_away_from_the_river() {
        let (width, height) = (16usize, 12usize);
        let c = 4usize;
        let mut grid = Grid::new(width, height);
        let main: Vec<usize> = (0..height).map(|row| row * width + c).collect();
        seed_river_side(&mut grid, &[main]);

        let row = 6;
        let on_river = grid.river_dist(row * width + c);
        let near = grid.river_dist(row * width + c + 2);
        let far = grid.river_dist(row * width + (width - 1));

        assert_eq!(on_river, 0.0, "a cell on the river has zero distance");
        assert!(near > on_river, "a cell off the river is farther than the river");
        assert!(far > near, "a more distant cell reads farther");
        assert!((far - 1.0).abs() < 1e-6, "the farthest cell normalizes to 1");
    }

    #[test]
    fn is_east_is_strict() {
        assert!(is_east(6, 5));
        assert!(!is_east(5, 5));
        assert!(!is_east(4, 5));
    }
}
