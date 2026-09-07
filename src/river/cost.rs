//! Anisotropic least-cost engine: smoothed cost field + Dijkstra with a directional penalty.

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

fn signed_noise(width: usize, height: usize, passes: usize, rng: &mut StdRng) -> Vec<f32> {
    let n = field::normalize(&field::value_noise(width, height, passes, rng));
    n.iter().map(|v| v * 2.0 - 1.0).collect()
}

// 8-connected so the carve follows warped valleys at their true angle, not axis-aligned staircases.
const NEIGHBORS: [(i32, i32); 8] = [
    (-1, 0), (1, 0), (0, -1), (0, 1),
    (-1, -1), (1, -1), (-1, 1), (1, 1),
];

// √2 per-mille: without scaling diagonals are cheaper per unit length, collapsing paths onto 45° lines.
const DIAG_NUM: u32 = 1414;
const DIAG_DEN: u32 = 1000;

// Against Heading::Down means dy == -1; sideways/diagonal-down carry no directional penalty.
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
                let step_cost = if dx != 0 && dy != 0 {
                    cost[n].saturating_mul(DIAG_NUM) / DIAG_DEN
                } else {
                    cost[n]
                };
                let dir_pen: u32 = match &bias {
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
