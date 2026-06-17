//! The water-proximity field: a multi-source BFS out from every water cell that
//! converts ring-distance into a grass carrying-capacity falloff.

use std::collections::VecDeque;

use crate::grid::Grid;

/// Multi-source BFS out from every water cell, converting ring-distance into a
/// grass carrying capacity: 0 in water, 1 right beside it, linearly down to 0 at
/// `reach` cells away.
pub(super) fn compute_water_prox(grid: &mut Grid, reach: u32) {
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
