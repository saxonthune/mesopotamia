//! The water-proximity field: a multi-source BFS out from every water cell that
//! converts ring-distance into a grass carrying-capacity falloff. Each source
//! carries its own reach — `lake_reach` for lake cells, `reach` for river and
//! tributary channels — so lakes fade to no grass over a shorter span and carry a
//! shallower bank than a river of the same depth.

use std::collections::VecDeque;

use crate::grid::Grid;

/// Multi-source BFS out from every water cell, converting ring-distance into a
/// grass carrying capacity: 0 in water, 1 right beside it, linearly down to 0 at
/// the source's reach away. A cell inherits the reach of whichever source reaches
/// it first, so lake banks (`lake_reach`) taper faster than river banks (`reach`).
pub(super) fn compute_water_prox(grid: &mut Grid, reach: u32, lake_reach: u32) {
    let len = grid.len();
    let mut dist = vec![u32::MAX; len];
    // The reach of the nearest water source, propagated outward with the BFS so a
    // cell falls off over its source's span rather than one global reach.
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
            // Distance 0 (on water) and unreached cells (no water anywhere) carry
            // no grass; src_reach is 0 there, so guard the division.
            0 | u32::MAX => 0.0,
            d => (1.0 - (d as f32 - 1.0) / src_reach[i] as f32).max(0.0),
        };
        grid.set_water_prox(i, prox);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Lakes carry a shorter bank than rivers: with `lake_reach < reach`, the grass
    /// capacity beside a lake fades to zero in fewer rings than beside a river.
    #[test]
    fn lake_banks_are_shallower_than_river_banks() {
        // A 1-row strip: a river cell at the left end, a lake cell at the right end.
        let width = 24usize;
        let mut grid = Grid::new(width, 1);
        grid.set_water(0, 1.0); // river at col 0 (not tagged lake)
        let lake_col = width - 1;
        grid.set_water(lake_col, 1.0);
        grid.set_lake(lake_col, true);

        compute_water_prox(&mut grid, 4, 2);

        // Three cells out: the river bank still carries grass, the lake bank is dry.
        assert!(grid.water_prox(3) > 0.0, "river bank should still carry grass at d=3");
        assert_eq!(
            grid.water_prox(lake_col - 3),
            0.0,
            "lake bank should be dry by d=3 with lake_reach=2"
        );
        // Water cells themselves never carry grass.
        assert_eq!(grid.water_prox(0), 0.0, "river water cell carries no grass");
        assert_eq!(grid.water_prox(lake_col), 0.0, "lake water cell carries no grass");
    }
}
