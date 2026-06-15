use bevy::prelude::*;
use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;
use std::collections::{BinaryHeap, VecDeque};
use std::cmp::Reverse;

use crate::grid::Grid;

const RIVER_SEED: u64 = 0xBEDA;
const SMOOTH_PASSES: usize = 8;
const RIVER_RADIUS: isize = 3;
const RIVER_CORE: isize = 1; // full-water core radius => minimum width 2*1+1 = 3
const WATER_REACH: u32 = 8; // cells of grass nourishment before capacity hits 0

fn generate_river(mut grid: ResMut<Grid>) {
    let mut rng = StdRng::seed_from_u64(RIVER_SEED);
    let cost = cost_field(&grid, &mut rng, SMOOTH_PASSES);
    let start = rng.random_range(0..grid.height()) * grid.width();
    let goal = rng.random_range(0..grid.height()) * grid.width() + (grid.width() -1);
    let centerline = carve(&grid, &cost, start, goal);

    rasterize(&mut grid, &centerline, RIVER_RADIUS);
    compute_water_prox(&mut grid, WATER_REACH);
}

/// Multi-source BFS out from every water cell, converting ring-distance into a
/// grass carrying capacity: 0 in water, 1 right beside it, linearly down to 0 at
/// `reach` cells away.
fn compute_water_prox(grid: &mut Grid, reach: u32) {
    let len = grid.len();
    let mut dist = vec![u32::MAX; len];
    let mut queue = VecDeque::new();

    for index in 0..len {
        if grid.water(index) > 0.0 {
            dist[index] = 0;
            queue.push_back(index);
        }
    }

    while let Some(index) = queue.pop_front() {
        let d = dist[index];
        for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
            if let Some(n) = grid.step(index, dx, dy) {
                if dist[n] == u32::MAX {
                    dist[n] = d + 1;
                    queue.push_back(n);
                }
            }
        }
    }

    for index in 0..len {
        let prox = match dist[index] {
            0 => 0.0,
            d => (1.0 - (d as f32 - 1.0) / reach as f32).max(0.0),
        };
        grid.set_water_prox(index, prox);
    }
}

pub struct RiverPlugin;

impl Plugin for RiverPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, generate_river);
    }
}

fn cost_field(grid: &Grid, rng: &mut StdRng, passes: usize) -> Vec<u32> {
    let len = grid.len();
    let mut field: Vec<f32> = 
        (0..len).map(|_| rng.random::<f32>()).collect();
    for _ in 0..passes {
        let mut next = field.clone();
        for index in 0..len {
            let mut sum = field[index];
            let mut count = 1.0;
            for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                if let Some(n) = grid.step(index, dx, dy) {
                    sum += field[n];
                    count += 1.0;
                }
            }
            next[index] = sum / count;
        }
        field = next;
    }
    field.iter().map(|v| (v * 1000.0) as u32 + 1).collect()
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

fn rasterize(grid: &mut Grid, centerline: &[usize], radius: isize) {
    for &center in centerline {
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                if let Some(cell) = grid.step(center, dx, dy) {
                    let dist = ((dx * dx + dy * dy) as f32).sqrt();
                    let level = if dist <= RIVER_CORE as f32 {
                        1.0
                    } else {
                        (1.0 - dist / (radius as f32 + 1.0)).max(0.0)
                    };
                    if level > grid.water(cell) {
                        grid.set_water(cell, level);
                    }
                }
            }
        }
    }
}
