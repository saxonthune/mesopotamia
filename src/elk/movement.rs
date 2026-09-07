//! Pure forage and crossing scorers — no ECS, no RNG, no per-tick state (doc04.01).

use bevy::prelude::*;

use crate::grid::Grid;

use super::components::ElkParams;

pub fn step_water_penalty(water: f32, is_ford: bool, water_cost: f32, ford_discount: f32) -> f32 {
    let base = water * water_cost;
    if is_ford { base * ford_discount } else { base }
}

/// Returns un-normalized direction — magnitude encodes pull strength.
pub fn grass_gradient(cell: usize, grid: &Grid, params: &ElkParams) -> Vec2 {
    let gr = params.grass_radius.ceil() as isize;
    let mut dir = Vec2::ZERO;
    for dy in -gr..=gr {
        for dx in -gr..=gr {
            if dx == 0 && dy == 0 {
                continue;
            }
            if let Some(n) = grid.step(cell, dx, dy) {
                let off = Vec2::new(dx as f32, dy as f32);
                let dist = off.length();
                if dist > params.grass_radius {
                    continue;
                }
                let attract = grid.forage(n) + params.freshness_weight * grid.freshness(n);
                dir += off / dist * (attract / dist);
            }
        }
    }
    dir
}

/// Long-range eastward pull beyond `grass_radius`. Returns a +x vector proportional to
/// how much the best ahead cell beats underfoot; zero when nothing ahead is richer.
pub fn forage_sightline(cell: usize, grid: &Grid, params: &ElkParams) -> Vec2 {
    if params.sightline_weight <= 0.0 || params.sightline_range < 1.0 {
        return Vec2::ZERO;
    }
    let attract = |c: usize| grid.forage(c) + params.freshness_weight * grid.freshness(c);
    let here = attract(cell);
    let mut best = here;
    let r = params.sightline_range.ceil() as isize;
    for dx in 1..=r {
        match grid.step(cell, dx, 0) {
            Some(n) => best = best.max(attract(n)),
            None => break, // ran off the grid — nothing further east to see
        }
    }
    let surplus = (best - here).max(0.0);
    Vec2::X * params.sightline_weight * surplus
}

pub fn cell_water_penalty(cell: usize, grid: &Grid, params: &ElkParams) -> f32 {
    step_water_penalty(grid.water(cell), grid.is_ford(cell), params.water_cost, params.ford_discount)
}

/// Positive when the far bank beats the best dry option net of crossing cost.
pub fn cross_desire(here: f32, ahead: f32, across: f32, cross_cost: f32) -> f32 {
    across - here.max(ahead) - cross_cost
}

const WATER_EPS: f32 = 0.01;

const SWIM_SCALE: f32 = 4.0;

/// Saturating cost: rises from 0 toward `reluctance` as span widens — wide rivers asymptote, streams are cheap.
pub fn swim_cost(effective_cells: f32, reluctance: f32) -> f32 {
    reluctance * (1.0 - (-effective_cells.max(0.0) / SWIM_SCALE).exp())
}

/// Ford-weighted walk from `cell` in `(dx, dy)`; returns (far_bank_cell, effective_cells) at
/// the first dry cell, letting the herd "see" forage beyond the grass-gradient radius.
pub fn forage_across(
    grid: &Grid,
    cell: usize,
    dx: isize,
    dy: isize,
    ford_discount: f32,
    max_peek: usize,
) -> Option<(usize, f32)> {
    let mut effective_cells = 0.0;
    let mut at = cell;
    let mut crossed_water = false;
    for _ in 0..max_peek {
        let next = grid.step(at, dx, dy)?;
        let water = grid.water(next);
        if water < WATER_EPS {
            return crossed_water.then_some((next, effective_cells));
        }
        effective_cells += if grid.is_ford(next) { ford_discount } else { 1.0 };
        crossed_water = true;
        at = next;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_params() -> ElkParams {
        ElkParams::default()
    }

    #[test]
    fn ford_penalty_is_near_zero() {
        let cost = step_water_penalty(1.0, true, 2.0, 0.1);
        assert!(cost < 0.25, "ford should be nearly free, got {cost}");
    }

    #[test]
    fn non_ford_penalty_equals_water_times_cost() {
        let cost = step_water_penalty(0.7, false, 2.0, 0.1);
        assert!((cost - 0.7 * 2.0).abs() < 1e-6);
    }

    #[test]
    fn penalty_monotone_in_water() {
        let low = step_water_penalty(0.2, false, 2.0, 0.1);
        let high = step_water_penalty(0.8, false, 2.0, 0.1);
        assert!(high > low);
    }

    #[test]
    fn deep_non_ford_beats_shallow_tributary() {
        let tributary = step_water_penalty(0.1, false, 2.0, 0.1);
        let deep = step_water_penalty(1.0, false, 2.0, 0.1);
        assert!(deep > tributary);
    }

    fn flat_grid(w: usize, h: usize, forage: f32) -> super::super::super::grid::Grid {
        let mut g = crate::grid::Grid::new(w, h);
        for i in 0..g.len() {
            g.set_grass(i, forage);
        }
        g
    }

    #[test]
    fn gradient_zero_on_flat_forage() {
        let grid = flat_grid(21, 21, 0.5);
        let params = default_params();
        let center = 10 * 21 + 10;
        let g = grass_gradient(center, &grid, &params);
        assert!(g.length() < 1e-4, "flat forage must yield zero gradient, got {g:?}");
    }

    #[test]
    fn gradient_points_toward_richer_forage() {
        let mut grid = crate::grid::Grid::new(21, 21);
        let center = 10 * 21 + 10;
        let right = center + 1;
        grid.set_grass(right, 1.0);
        let params = default_params();
        let g = grass_gradient(center, &grid, &params);
        assert!(g.x > 0.0, "gradient must point toward richer forage (+x), got {g:?}");
        assert!(g.x.abs() > g.y.abs(), "gradient must be predominantly rightward");
    }

    #[test]
    fn gradient_magnitude_rises_with_neighbor_forage() {
        let mut grid = crate::grid::Grid::new(21, 21);
        let center = 10 * 21 + 10;
        let right = center + 1;
        let params = default_params();

        grid.set_grass(right, 0.3);
        let low = grass_gradient(center, &grid, &params).length();

        grid.set_grass(right, 0.9);
        let high = grass_gradient(center, &grid, &params).length();

        assert!(high > low, "richer forage must raise gradient magnitude");
    }

    #[test]
    fn gradient_points_toward_fresh_over_mature_biomass() {
        let mut grid = crate::grid::Grid::new(21, 21);
        let center = 10 * 21 + 10;
        let left = center - 1;
        let right = center + 1;

        grid.set_grass(left, 0.9);
        grid.set_grass(right, 0.3);
        grid.freshness[right] = 2.0;

        let mut params = default_params();
        params.freshness_weight = 1.5;

        let g = grass_gradient(center, &grid, &params);
        assert!(
            g.x > 0.0,
            "with freshness_weight > 0, fresh cell (+x) must beat mature (-x): gradient={g:?}"
        );
    }

    #[test]
    fn gradient_freshness_weight_zero_is_identity() {
        let mut grid = crate::grid::Grid::new(21, 21);
        let center = 10 * 21 + 10;
        let right = center + 1;
        grid.set_grass(right, 0.8);
        grid.freshness[right] = 5.0;

        let mut params_zero = default_params();
        params_zero.freshness_weight = 0.0;

        let mut params_positive = default_params();
        params_positive.freshness_weight = 0.0;

        let g_zero = grass_gradient(center, &grid, &params_zero);
        let g_pos = grass_gradient(center, &grid, &params_positive);

        assert!(
            (g_zero - g_pos).length() < 1e-6,
            "freshness_weight=0 must yield identical result regardless of freshness field"
        );
    }

    #[test]
    fn sightline_off_by_default_is_zero() {
        let mut grid = crate::grid::Grid::new(40, 5);
        let center = 2 * 40 + 5;
        grid.set_grass(center + 15, 1.0);
        let params = default_params();
        assert_eq!(forage_sightline(center, &grid, &params), Vec2::ZERO);
    }

    #[test]
    fn sightline_pulls_east_toward_far_forage() {
        let mut grid = crate::grid::Grid::new(40, 5);
        let center = 2 * 40 + 5;
        grid.set_grass(center + 12, 1.0); // 12 cells east — past grass_radius 5
        let mut params = default_params();
        params.sightline_range = 16.0;
        params.sightline_weight = 1.0;
        let s = forage_sightline(center, &grid, &params);
        assert!(s.x > 0.0 && s.y == 0.0, "must pull purely east toward far forage: {s:?}");
    }

    #[test]
    fn sightline_zero_when_nothing_better_ahead() {
        let mut grid = crate::grid::Grid::new(40, 5);
        let center = 2 * 40 + 5;
        grid.set_grass(center, 1.0);
        let mut params = default_params();
        params.sightline_range = 16.0;
        params.sightline_weight = 1.0;
        assert_eq!(forage_sightline(center, &grid, &params), Vec2::ZERO);
    }

    #[test]
    fn sightline_follows_the_freshness_front() {
        let mut grid = crate::grid::Grid::new(40, 5);
        let center = 2 * 40 + 5;
        let front = center + 10;
        grid.freshness[front] = 3.0;
        let mut params = default_params();
        params.sightline_range = 16.0;
        params.sightline_weight = 1.0;
        params.freshness_weight = 1.5;
        assert!(forage_sightline(center, &grid, &params).x > 0.0);
    }

    #[test]
    fn lead_herd_with_forage_ahead_does_not_cross() {
        assert!(cross_desire(0.5, 0.9, 0.9, 0.2) <= 0.0);
    }

    #[test]
    fn trailing_herd_crosses_for_the_far_bank() {
        assert!(cross_desire(0.1, 0.1, 0.9, 0.2) > 0.0);
    }

    #[test]
    fn more_forage_ahead_never_raises_cross_desire() {
        assert!(cross_desire(0.1, 0.8, 0.9, 0.2) <= cross_desire(0.1, 0.2, 0.9, 0.2));
    }

    #[test]
    fn more_forage_across_never_lowers_cross_desire() {
        assert!(cross_desire(0.1, 0.1, 0.9, 0.2) >= cross_desire(0.1, 0.1, 0.4, 0.2));
    }

    #[test]
    fn costlier_crossing_never_raises_cross_desire() {
        assert!(cross_desire(0.1, 0.1, 0.9, 0.5) <= cross_desire(0.1, 0.1, 0.9, 0.1));
    }

    fn river_row() -> Grid {
        let mut grid = Grid::new(6, 1);
        grid.set_water(1, 1.0);
        grid.set_water(2, 1.0);
        grid.set_grass(3, 0.5);
        grid
    }

    #[test]
    fn forage_across_finds_far_bank_and_counts_cells() {
        let grid = river_row();
        let (far_cell, cells) = forage_across(&grid, 0, 1, 0, 0.1, 12).unwrap();
        assert_eq!(far_cell, 3);
        assert!((cells - 2.0).abs() < 1e-6);
    }

    #[test]
    fn forage_across_is_none_when_step_is_dry() {
        let mut grid = Grid::new(6, 1);
        grid.set_grass(1, 0.5);
        assert!(forage_across(&grid, 0, 1, 0, 0.1, 12).is_none());
    }

    #[test]
    fn forage_across_is_none_when_far_bank_out_of_reach() {
        let mut grid = Grid::new(6, 1);
        for i in 1..6 {
            grid.set_water(i, 1.0);
        }
        assert!(forage_across(&grid, 0, 1, 0, 0.1, 12).is_none());
    }

    #[test]
    fn forage_across_counts_fords_as_fractional_cells() {
        let mut grid = river_row();
        grid.set_ford(1, true);
        grid.set_ford(2, true);
        let (_, cells) = forage_across(&grid, 0, 1, 0, 0.1, 12).unwrap();
        assert!((cells - 0.2).abs() < 1e-6, "ford span should count 2×0.1 = 0.2, got {cells}");
    }

    #[test]
    fn swim_cost_is_zero_for_no_water() {
        assert!(swim_cost(0.0, 0.6).abs() < 1e-6);
    }

    #[test]
    fn swim_cost_is_bounded_by_reluctance() {
        for cells in [1.0_f32, 4.0, 12.0, 50.0, 1000.0] {
            let c = swim_cost(cells, 0.6);
            assert!(c < 0.6 + 1e-6, "cost {c} exceeds reluctance at {cells} cells");
        }
        assert!(swim_cost(1000.0, 0.6) > 0.6 - 1e-3);
    }

    #[test]
    fn swim_cost_rises_with_width() {
        let widths = [0.0_f32, 1.0, 2.0, 4.0, 8.0, 16.0];
        let costs: Vec<f32> = widths.iter().map(|&w| swim_cost(w, 0.6)).collect();
        for w in costs.windows(2) {
            assert!(w[1] >= w[0], "cost must not fall as width rises: {} < {}", w[1], w[0]);
        }
    }

    #[test]
    fn swim_cost_makes_a_narrow_ford_cheaper_than_a_wide_river() {
        let ford = swim_cost(0.4, 0.6);
        let river = swim_cost(14.0, 0.6);
        assert!(ford < river * 0.5, "a ford ({ford}) should be much cheaper than a river ({river})");
    }
}
