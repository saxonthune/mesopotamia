//! Herd-level resource-abundance metrics (view state only — nothing in the sim reads these).

use bevy::prelude::*;

use crate::grid::{Grid, GrowthRate};

#[derive(Resource)]
pub struct AbundanceParams {
    pub radius: f32,
    /// Blend: `w·grass + (1-w)·energy`; 1.0 = pure grass, 0.0 = pure energy.
    pub energy_weight: f32,
}

impl Default for AbundanceParams {
    fn default() -> Self {
        Self { radius: 8.0, energy_weight: 1.0 }
    }
}

pub fn local_abundance(grass: f32, energy: f32, weight: f32) -> f32 {
    let w = weight.clamp(0.0, 1.0);
    w * grass + (1.0 - w) * energy
}

pub fn per_capita(total: f32, count: u32) -> f32 {
    if count == 0 { 0.0 } else { total / count as f32 }
}

/// Above 1: patch refills faster than the herd strips it — herd has no reason to move.
pub fn regrowth_drain_ratio(regrowth_per_elk: f32, drain: f32) -> f32 {
    if drain <= 0.0 { 0.0 } else { regrowth_per_elk / drain }
}

/// Sum of standing grass within Euclidean `radius` of `(col, row)`.
pub fn grass_in_radius(grid: &Grid, col: usize, row: usize, radius: f32) -> f32 {
    sum_in_radius(grid, col, row, radius, |g, i| g.grass(i))
}

/// Sum of `intrinsic·(capacity − grass)` within `radius` — bare regrowth, no spread term.
pub fn regrowth_in_radius(
    grid: &Grid,
    growth: &GrowthRate,
    col: usize,
    row: usize,
    radius: f32,
) -> f32 {
    sum_in_radius(grid, col, row, radius, |g, i| {
        growth.intrinsic * (g.capacity(i) - g.grass(i)).max(0.0)
    })
}

fn sum_in_radius(
    grid: &Grid,
    col: usize,
    row: usize,
    radius: f32,
    f: impl Fn(&Grid, usize) -> f32,
) -> f32 {
    let r = radius.max(0.0).floor() as isize;
    let r2 = radius * radius;
    let (w, h) = (grid.width() as isize, grid.height() as isize);
    let mut sum = 0.0;
    for dy in -r..=r {
        for dx in -r..=r {
            if (dx * dx + dy * dy) as f32 > r2 {
                continue;
            }
            let c = col as isize + dx;
            let rr = row as isize + dy;
            if c < 0 || rr < 0 || c >= w || rr >= h {
                continue;
            }
            sum += f(grid, rr as usize * grid.width() + c as usize);
        }
    }
    sum
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abundance_blends_toward_grass_at_weight_one() {
        assert_eq!(local_abundance(10.0, 2.0, 1.0), 10.0);
        assert_eq!(local_abundance(10.0, 2.0, 0.0), 2.0);
        assert_eq!(local_abundance(10.0, 2.0, 0.5), 6.0);
    }

    #[test]
    fn abundance_clamps_out_of_range_weight() {
        assert_eq!(local_abundance(10.0, 2.0, 5.0), 10.0);
        assert_eq!(local_abundance(10.0, 2.0, -1.0), 2.0);
    }

    #[test]
    fn per_capita_divides_and_guards_empty_herd() {
        assert_eq!(per_capita(12.0, 4), 3.0);
        assert_eq!(per_capita(12.0, 0), 0.0);
    }

    #[test]
    fn ratio_above_one_means_surplus() {
        assert!(regrowth_drain_ratio(0.008, 0.004) > 1.0);
        assert!(regrowth_drain_ratio(0.002, 0.004) < 1.0);
    }

    #[test]
    fn ratio_guards_zero_drain() {
        assert_eq!(regrowth_drain_ratio(0.01, 0.0), 0.0);
    }

    #[test]
    fn grass_in_radius_zero_is_center_cell_only() {
        let mut grid = Grid::new(5, 5);
        grid.set_grass(2 * 5 + 2, 0.5); // center
        grid.set_grass(2 * 5 + 3, 0.5); // neighbour
        assert_eq!(grass_in_radius(&grid, 2, 2, 0.0), 0.5);
    }

    #[test]
    fn grass_in_radius_one_includes_orthogonal_neighbours() {
        let mut grid = Grid::new(5, 5);
        // Orthogonal neighbours are within radius 1; diagonals (dist √2) are not.
        for &cell in &[2 * 5 + 2, 2 * 5 + 1, 2 * 5 + 3, 1 * 5 + 2, 3 * 5 + 2] {
            grid.set_grass(cell, 0.2);
        }
        grid.set_grass(1 * 5 + 1, 0.2); // a diagonal — must be excluded
        let sum = grass_in_radius(&grid, 2, 2, 1.0);
        assert!((sum - 1.0).abs() < 1e-6, "expected 5×0.2 = 1.0, got {sum}");
    }

    #[test]
    fn grass_in_radius_clamps_to_grid_edge() {
        let mut grid = Grid::new(3, 3);
        grid.set_grass(0, 0.4); // corner; most of the radius box is off-grid
        assert_eq!(grass_in_radius(&grid, 0, 0, 1.0), 0.4);
    }
}
