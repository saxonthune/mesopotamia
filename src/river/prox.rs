//! Water-proximity field: BFS from water cells converting ring-distance to grass capacity.
//! `lake_reach < reach` so lake banks taper faster than river banks.

use std::collections::VecDeque;

use crate::grid::Grid;

pub(super) fn compute_water_prox(grid: &mut Grid, reach: u32, lake_reach: u32) {
    let len = grid.len();
    let mut dist = vec![u32::MAX; len];
    let mut src_reach = vec![0u32; len];
    let mut queue = VecDeque::new();

    for i in 0..len {
        if grid.water(i) > 0.0 {
            dist[i] = 0;
            src_reach[i] = if grid.is_lake(i) { lake_reach } else { reach };
            queue.push_back(i);
        }
    }

    while let Some(index) = queue.pop_front() {
        let d = dist[index];
        let r = src_reach[index];
        for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
            if let Some(n) = grid.step(index, dx, dy) && dist[n] == u32::MAX {
                dist[n] = d + 1;
                src_reach[n] = r;
                queue.push_back(n);
            }
        }
    }

    for i in 0..len {
        let prox = match dist[i] {
            // src_reach is 0 on water/unreached cells; guard the division.
            0 | u32::MAX => 0.0,
            d => (1.0 - (d as f32 - 1.0) / src_reach[i] as f32).max(0.0),
        };
        grid.set_water_prox(i, prox);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lake_banks_are_shallower_than_river_banks() {
        let width = 24usize;
        let mut grid = Grid::new(width, 1);
        grid.set_water(0, 1.0); // river at col 0 (not tagged lake)
        let lake_col = width - 1;
        grid.set_water(lake_col, 1.0);
        grid.set_lake(lake_col, true);

        compute_water_prox(&mut grid, 4, 2);

        assert!(grid.water_prox(3) > 0.0, "river bank should still carry grass at d=3");
        assert_eq!(
            grid.water_prox(lake_col - 3),
            0.0,
            "lake bank should be dry by d=3 with lake_reach=2"
        );
        assert_eq!(grid.water_prox(0), 0.0, "river water cell carries no grass");
        assert_eq!(grid.water_prox(lake_col), 0.0, "lake water cell carries no grass");
    }
}
