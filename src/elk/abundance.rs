//! Herd-level resource-abundance metrics.
//!
//! These quantify whether a herd's local patch supplies more than it consumes —
//! the mechanism behind elk that camp and graze instead of migrating. A herd whose
//! nearby forage regrows faster than it is eaten has no depletion gradient pushing
//! it onward, so it stays put. Measuring that surplus is how we prove the herd has
//! no reason to migrate.
//!
//! Measured at the herd (centroid + radius) rather than per elk, so the cost is a
//! handful of grid scans per tick instead of one per animal. Pure view state:
//! nothing in the simulation reads these, so changing the tunables never perturbs
//! behaviour.

use bevy::prelude::*;

use crate::grid::{Grid, GrowthRate};

/// Tunables for the abundance measurement.
#[derive(Resource)]
pub struct AbundanceParams {
    /// Sampling radius (in cells) around a herd's centroid for "nearby" forage.
    pub radius: f32,
    /// Blend weight for the composite abundance: `w·grass + (1-w)·energy`.
    /// 1.0 = pure nearby grass, 0.0 = pure herd energy.
    pub energy_weight: f32,
}

impl Default for AbundanceParams {
    fn default() -> Self {
        Self { radius: 8.0, energy_weight: 1.0 }
    }
}

/// Composite local abundance: a slider-weighted blend of nearby grass and the
/// herd's stored energy. `weight` is clamped to [0, 1]; 1 weights grass fully.
pub fn local_abundance(grass: f32, energy: f32, weight: f32) -> f32 {
    let w = weight.clamp(0.0, 1.0);
    w * grass + (1.0 - w) * energy
}

/// Split a herd total across its members. An empty herd has no per-capita value.
pub fn per_capita(total: f32, count: u32) -> f32 {
    if count == 0 { 0.0 } else { total / count as f32 }
}

/// Ratio of per-elk forage regrowth to per-elk metabolic drain. Above 1 the local
/// patch refills faster than the herd strips it, so the herd can camp indefinitely
/// — the quantitative signature of "grass too plentiful / regrows too fast".
pub fn regrowth_drain_ratio(regrowth_per_elk: f32, drain: f32) -> f32 {
    if drain <= 0.0 { 0.0 } else { regrowth_per_elk / drain }
}

/// Sum of standing grass over every cell within `radius` of `(col, row)`.
/// Pure over the grid; the scan box is clamped to the grid bounds and uses a
/// circular (Euclidean) cutoff so "nearby" is isotropic.
pub fn grass_in_radius(grid: &Grid, col: usize, row: usize, radius: f32) -> f32 {
    sum_in_radius(grid, col, row, radius, |g, i| g.grass(i))
}

/// Sum of intrinsic regrowth `intrinsic·(capacity − grass)` over every cell within
/// `radius` — the forage the patch adds next tick from bare growth alone (the
/// `spread` term needs neighbour cover and is omitted as a lower bound).
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

/// Accumulate `f` over the cells within a circular `radius` of `(col, row)`,
/// clamped to the grid. Shared by the grass and regrowth scans.
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
        // Regrowth out-paces drain: the patch refills faster than it's eaten.
        assert!(regrowth_drain_ratio(0.008, 0.004) > 1.0);
        // Regrowth lags drain: the herd would have to move.
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
        // Center + its four orthogonal neighbours are within Euclidean radius 1;
        // the diagonals (dist √2) are not.
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
