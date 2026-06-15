use bevy::prelude::*;
use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;
use std::collections::{BinaryHeap, VecDeque};
use std::cmp::Reverse;

use crate::field;
use crate::grid::Grid;

const RIVER_SEED: u64 = 0xBEDA;
/// Fewer smoothing passes → tighter wiggles in the cost field → more sinuous channel.
const MEANDER_PASSES: usize = 4;
const RIVER_RADIUS: isize = 3;
const RIVER_CORE: isize = 1;
const WATER_REACH: u32 = 8;

const TRIB_COUNT: usize = 2;
const TRIB_RADIUS: isize = 1;
const TRIB_DEPTH: f32 = 0.35;

const OXBOW_COUNT: usize = 2;
const OXBOW_RADIUS: isize = 1;
const OXBOW_DEPTH: f32 = 0.3;

const FORD_SPACING: usize = 20;
const FORD_DEPTH: f32 = 0.3;

fn generate_river(mut grid: ResMut<Grid>) {
    generate_river_inner(&mut grid);
}

fn generate_river_inner(grid: &mut Grid) {
    let mut rng = StdRng::seed_from_u64(RIVER_SEED);
    let cost = cost_field(grid, &mut rng, MEANDER_PASSES);

    // Main channel — full radius, deep core (max_depth 1.0).
    let start = rng.random_range(0..grid.height()) * grid.width();
    let goal = rng.random_range(0..grid.height()) * grid.width() + (grid.width() - 1);
    let centerline = carve(grid, &cost, start, goal);
    rasterize(grid, &centerline, RIVER_RADIUS, 1.0);

    // Shallow narrow tributaries — same cost field, distinct start/goal rows.
    for _ in 0..TRIB_COUNT {
        let ts = rng.random_range(0..grid.height()) * grid.width();
        let tg = rng.random_range(0..grid.height()) * grid.width() + (grid.width() - 1);
        let tcl = carve(grid, &cost, ts, tg);
        rasterize(grid, &tcl, TRIB_RADIUS, TRIB_DEPTH);
    }

    // Oxbow pools: local cost minima not adjacent to any carved channel cell.
    let mut cands: Vec<usize> = (0..grid.len())
        .filter(|&i| {
            grid.water(i) == 0.0
                && [(-1, 0), (1, 0), (0, -1i32), (0, 1i32)]
                    .iter()
                    .all(|&(dx, dy)| grid.step(i, dx as isize, dy as isize).is_none_or(|n| grid.water(n) == 0.0))
                && [(-1, 0), (1, 0), (0, -1i32), (0, 1i32)]
                    .iter()
                    .all(|&(dx, dy)| grid.step(i, dx as isize, dy as isize).is_none_or(|n| cost[n] >= cost[i]))
        })
        .collect();
    for _ in 0..OXBOW_COUNT.min(cands.len()) {
        let ci = rng.random_range(0..cands.len());
        let center = cands.swap_remove(ci);
        rasterize(grid, &[center], OXBOW_RADIUS, OXBOW_DEPTH);
    }

    // Fords: periodic shallow crossing bands along the main channel only.
    for &ford_center in &ford_indices(&centerline, FORD_SPACING) {
        for dy in -RIVER_RADIUS..=RIVER_RADIUS {
            if let Some(cell) = grid.step(ford_center, 0, dy) {
                grid.set_water(cell, FORD_DEPTH);
                grid.set_ford(cell, true);
            }
        }
    }

    compute_water_prox(grid, WATER_REACH);
}

/// Returns every `spacing`-th element of `centerline` by position (0, spacing, 2*spacing, …).
fn ford_indices(centerline: &[usize], spacing: usize) -> Vec<usize> {
    centerline.iter().copied().step_by(spacing).collect()
}

/// Multi-source BFS out from every water cell, converting ring-distance into a
/// grass carrying capacity: 0 in water, 1 right beside it, linearly down to 0 at
/// `reach` cells away.
fn compute_water_prox(grid: &mut Grid, reach: u32) {
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

pub struct RiverPlugin;

impl Plugin for RiverPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, generate_river);
    }
}

/// Build the white-noise cost field and smooth it `passes` times via `field::smooth`.
fn cost_field(grid: &Grid, rng: &mut StdRng, passes: usize) -> Vec<u32> {
    let white: Vec<f32> = (0..grid.len()).map(|_| rng.random::<f32>()).collect();
    let smoothed = field::smooth(&white, grid.width(), grid.height(), passes);
    smoothed.iter().map(|v| (v * 1000.0) as u32 + 1).collect()
}

fn carve(grid: &Grid, cost: &[u32], start: usize, goal: usize) -> Vec<usize> {
    let len = grid.len();
    let mut dist = vec![u32::MAX; len];
    let mut prev = vec![usize::MAX; len];
    let mut heap = BinaryHeap::new();

    dist[start] = 0;
    heap.push(Reverse((0, start)));

    while let Some(Reverse((d, index))) = heap.pop() {
        if index == goal {
            break;
        }
        if d > dist[index] {
            continue;
        }
        for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
            if let Some(n) = grid.step(index, dx, dy) {
                let nd = d + cost[n];
                if nd < dist[n] {
                    dist[n] = nd;
                    prev[n] = index;
                    heap.push(Reverse((nd, n)))
                }
            }
        }
    }

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

/// Stamp water values around each centerline cell. `max_depth` scales the level:
/// 1.0 gives a full-depth main channel; lower values produce shallow streams/pools.
/// Uses max-merge so overlapping channels keep the deeper value.
fn rasterize(grid: &mut Grid, centerline: &[usize], radius: isize, max_depth: f32) {
    for &center in centerline {
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                if let Some(cell) = grid.step(center, dx, dy) {
                    let dist = ((dx * dx + dy * dy) as f32).sqrt();
                    let level = if dist <= RIVER_CORE as f32 {
                        max_depth
                    } else {
                        (max_depth * (1.0 - dist / (radius as f32 + 1.0))).max(0.0)
                    };
                    if level > grid.water(cell) {
                        grid.set_water(cell, level);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::Grid;

    #[test]
    fn ford_indices_returns_every_nth_cell() {
        let cl: Vec<usize> = (10..110).collect(); // 100 elements: 10..109
        let fords = ford_indices(&cl, 25);
        assert_eq!(fords, vec![10, 35, 60, 85]);
    }

    #[test]
    fn ford_indices_empty_centerline() {
        assert!(ford_indices(&[], 10).is_empty());
    }

    #[test]
    fn ford_indices_spacing_larger_than_len() {
        let cl = vec![7usize, 8, 9];
        let fords = ford_indices(&cl, 10);
        assert_eq!(fords, vec![7]); // only position 0
    }

    #[test]
    fn generation_is_deterministic() {
        let mut g1 = Grid::new(128, 32);
        let mut g2 = Grid::new(128, 32);
        generate_river_inner(&mut g1);
        generate_river_inner(&mut g2);
        let w1: Vec<f32> = (0..g1.len()).map(|i| g1.water(i)).collect();
        let w2: Vec<f32> = (0..g2.len()).map(|i| g2.water(i)).collect();
        assert_eq!(w1, w2, "same seed must produce identical water fields");
    }

    #[test]
    fn fords_carry_low_water() {
        let mut g = Grid::new(128, 32);
        generate_river_inner(&mut g);
        for i in 0..g.len() {
            if g.is_ford(i) {
                assert!(
                    g.water(i) <= FORD_DEPTH + f32::EPSILON,
                    "ford cell {i} has water {} > FORD_DEPTH {FORD_DEPTH}",
                    g.water(i)
                );
            }
        }
    }

    #[test]
    fn main_channel_has_deep_water() {
        let mut g = Grid::new(128, 32);
        generate_river_inner(&mut g);
        let deep_cells = (0..g.len()).filter(|&i| g.water(i) > 0.9).count();
        assert!(deep_cells > 0, "expected deep main-channel core cells");
    }

    #[test]
    fn water_levels_span_expected_range() {
        let mut g = Grid::new(128, 32);
        generate_river_inner(&mut g);
        let max_water = (0..g.len()).map(|i| g.water(i)).fold(0.0f32, f32::max);
        let has_shallow = (0..g.len()).any(|i| g.water(i) > 0.0 && g.water(i) < 0.5);
        assert!(max_water > 0.9, "expected deep main-channel water, got max={max_water}");
        assert!(has_shallow, "expected shallow tributary/ford cells");
    }
}
