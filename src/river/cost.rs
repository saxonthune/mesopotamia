//! The anisotropic least-cost engine. White noise is smoothed into a cost field
//! whose wavelength sets bend breadth; `carve` then runs Dijkstra over that field
//! with a directional penalty that biases the path along the flow heading.

use rand::Rng;
use rand::rngs::StdRng;
use std::cmp::Reverse;
use std::collections::BinaryHeap;

use crate::field;
use crate::grid::Grid;

use super::spec::Heading;

pub(super) fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

/// Build the white-noise cost field and smooth it `passes` times via `field::smooth`.
pub(super) fn cost_field(grid: &Grid, rng: &mut StdRng, passes: usize) -> Vec<u32> {
    let white: Vec<f32> = (0..grid.len()).map(|_| rng.random::<f32>()).collect();
    let smoothed = field::smooth(&white, grid.width(), grid.height(), passes);
    smoothed.iter().map(|v| (v * 1000.0) as u32 + 1).collect()
}

/// Dijkstra on a grid graph. When `bias` is `Some((Heading::Down, penalty))`,
/// steps that go against the heading (upward: dy = -1) pay an extra `penalty`
/// on top of the cell's noise cost, biasing the path to descend. Sideways steps
/// are free; the lateral drift is encoded in the goal offset, not here.
pub(super) fn carve(
    grid: &Grid,
    cost: &[u32],
    start: usize,
    goal: usize,
    bias: Option<(&Heading, u32)>,
) -> Vec<usize> {
    let len = grid.len();
    let mut dist = vec![u32::MAX; len];
    let mut prev = vec![usize::MAX; len];
    let mut heap = BinaryHeap::new();

    dist[start] = 0;
    heap.push(Reverse((0u32, start)));

    while let Some(Reverse((d, index))) = heap.pop() {
        if index == goal {
            break;
        }
        if d > dist[index] {
            continue;
        }
        for (dx, dy) in [(-1, 0), (1, 0), (0, -1i32), (0, 1i32)] {
            if let Some(n) = grid.step(index, dx as isize, dy as isize) {
                let dir_pen: u32 = match &bias {
                    // Against Heading::Down means going up (dy == -1).
                    Some((Heading::Down, penalty)) if dy == -1 => *penalty,
                    _ => 0,
                };
                let nd = d.saturating_add(cost[n]).saturating_add(dir_pen);
                if nd < dist[n] {
                    dist[n] = nd;
                    prev[n] = index;
                    heap.push(Reverse((nd, n)));
                }
            }
        }
    }

    // Reconstruct path from goal back to start; path[0] = goal, path.last() = start.
    let mut path = Vec::new();
    let mut cur = goal;
    while cur != usize::MAX {
        path.push(cur);
        if cur == start {
            break;
        }
        cur = prev[cur];
    }
    path
}
