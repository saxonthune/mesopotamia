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

/// Build the white-noise cost field, smooth it `passes` times via `field::smooth`,
/// then domain-warp it by two broad-wavelength noise fields so the straight valleys
/// the carve would otherwise follow bend into sinuous ones. `warp_amp` is the
/// displacement in cells (0 disables warping); `warp_passes` sets the warp
/// wavelength — more passes give broader, lower-frequency meanders.
pub(super) fn cost_field(
    grid: &Grid,
    rng: &mut StdRng,
    passes: usize,
    warp_amp: f32,
    warp_passes: usize,
) -> Vec<u32> {
    let (w, h) = (grid.width(), grid.height());
    let white: Vec<f32> = (0..grid.len()).map(|_| rng.random::<f32>()).collect();
    let base = field::smooth(&white, w, h, passes);

    let warped = if warp_amp > 0.0 {
        let wx = signed_noise(w, h, warp_passes, rng);
        let wy = signed_noise(w, h, warp_passes, rng);
        field::domain_warp(&base, w, h, &wx, &wy, warp_amp)
    } else {
        base
    };

    warped.iter().map(|v| (v * 1000.0) as u32 + 1).collect()
}

/// A smoothed value-noise field rescaled to the signed range [-1, 1] — the
/// displacement field a domain warp consumes.
fn signed_noise(width: usize, height: usize, passes: usize, rng: &mut StdRng) -> Vec<f32> {
    let n = field::normalize(&field::value_noise(width, height, passes, rng));
    n.iter().map(|v| v * 2.0 - 1.0).collect()
}

/// 8-connected neighbour offsets: the four cardinals followed by the four
/// diagonals. Diagonal moves let the carve follow a warped valley at its true
/// angle instead of quantizing it into axis-aligned staircases.
const NEIGHBORS: [(i32, i32); 8] = [
    (-1, 0), (1, 0), (0, -1), (0, 1),
    (-1, -1), (1, -1), (-1, 1), (1, 1),
];

/// √2 in per-mille — a diagonal step covers √2 cells, so its cost is scaled up by
/// this over the orthogonal cost to keep the metric isotropic. Without it diagonals
/// are cheaper per unit length and the path collapses onto 45° lines.
const DIAG_NUM: u32 = 1414;
const DIAG_DEN: u32 = 1000;

/// Dijkstra on an 8-connected grid graph. When `bias` is `Some((Heading::Down,
/// penalty))`, steps that go against the heading (any upward move, dy = -1) pay an
/// extra `penalty` on top of the cell's noise cost, biasing the path to descend.
/// Sideways and diagonal-down steps carry no directional penalty; the lateral drift
/// is encoded in the goal offset, not here. Diagonal steps pay √2× their cell cost.
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
        for (dx, dy) in NEIGHBORS {
            if let Some(n) = grid.step(index, dx as isize, dy as isize) {
                // Diagonal steps cover √2 cells, so scale the cell cost to match.
                let step_cost = if dx != 0 && dy != 0 {
                    cost[n].saturating_mul(DIAG_NUM) / DIAG_DEN
                } else {
                    cost[n]
                };
                let dir_pen: u32 = match &bias {
                    // Against Heading::Down means going up (dy == -1).
                    Some((Heading::Down, penalty)) if dy == -1 => *penalty,
                    _ => 0,
                };
                let nd = d.saturating_add(step_cost).saturating_add(dir_pen);
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
