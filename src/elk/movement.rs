//! The pure forage and river-crossing scorers — value and cost kernels with no ECS,
//! no RNG, and no per-tick state. The live herding model (`herding::herd_step`) and
//! the field overlays consume these; each is unit-tested in isolation against
//! hand-picked cases and metamorphic relations (doc04.01).

use bevy::prelude::*;

use crate::grid::Grid;

use super::components::ElkParams;

/// Step cost for entering a water cell. On a ford the cost is reduced by
/// `ford_discount`; off a ford it is the raw water-level × cost (today's behaviour).
pub fn step_water_penalty(water: f32, is_ford: bool, water_cost: f32, ford_discount: f32) -> f32 {
    let base = water * water_cost;
    if is_ford { base * ford_discount } else { base }
}

/// Grass-gradient field at `cell`: the pull-toward-forage vector the herding model
/// steers by, as a pure function of the grid and params. Nearby rich cells weigh
/// more; cells beyond `grass_radius` are ignored. Returns a raw (un-normalized)
/// direction vector — magnitude reflects how strongly forage pulls from each direction.
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

/// Long-range forward forage pull on dry land — the leapfrog primitive. Beyond
/// the local `grass_radius`, an elk on a depleted patch still steers toward a
/// clearly richer cell it can see ahead along the migration axis (+x): the
/// dry-land analogue of `forage_across` peeking over a river. Walks east up to
/// `sightline_range` cells, climbing the same attractiveness the local gradient
/// does (`forage + freshness_weight · freshness`, so the green-up front is the
/// long-range beacon), and returns a +x pull proportional to how much the best
/// cell ahead beats underfoot. Zero when nothing ahead is richer (no pull to
/// nowhere) or `sightline_weight == 0` (identity — the grass drive stays local).
///
/// Folded into the herding signal before it is normalized, so the tilt is *relative*:
/// on rich ground the strong local gradient dominates and the elk grazes; on
/// drawn-down ground the local gradient is weak and the sightline takes over,
/// pointing the herd east toward the next forage. That switch is the emergent
/// "stick to good grass, roll forward off bad" behaviour.
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

/// Water-penalty field at `cell`: the step cost an elk would pay to enter this
/// cell, sampled for overlay rendering. Thin wrapper so grid sampling does not
/// have to repeat the ford/cost arithmetic.
pub fn cell_water_penalty(cell: usize, grid: &Grid, params: &ElkParams) -> f32 {
    step_water_penalty(grid.water(cell), grid.is_ford(cell), params.water_cost, params.ford_discount)
}

/// The verifiable behavioural contract for river crossings (world-gen C): a herd's
/// net incentive to ford to the far bank rather than stay on its current side.
/// Forage terms are in [0, 1] — `here` under the herd, `ahead` the next forage along
/// its migration axis (+x), `across` the forage on the far bank (+y over the water);
/// `cross_cost` is the effective ford price. Positive ⇒ the far bank beats the best
/// dry option net of the crossing, so the herd wants to cross.
///
/// The model: a herd takes the richest *reachable* forage and only pays to cross when
/// nothing cheaper rivals the far bank — so a lead herd with fresh grass still ahead
/// stays, while a trailing herd facing a grazed-out corridor crosses. The metamorphic
/// tests below pin its behaviour, and `herding::crossing_ahead` consumes it directly.
pub fn cross_desire(here: f32, ahead: f32, across: f32, cross_cost: f32) -> f32 {
    across - here.max(ahead) - cross_cost
}

/// A water step entering a cell with at least this water level engages the
/// crossing incentive; below it the step is dry land and scored normally.
const WATER_EPS: f32 = 0.01;

/// Width scale (in effective water cells) of the crossing-cost saturation. A span
/// a few of these long already costs nearly the full `swim_reluctance`; beyond it
/// the cost barely grows. Sets how quickly a wider river stops mattering.
const SWIM_SCALE: f32 = 4.0;

/// The *decision* cost of committing to a crossing, saturating with span width.
/// `effective_cells` is the ford-weighted count of water cells to traverse (a ford
/// cell counts as `ford_discount` of a full cell). The cost rises from 0 toward
/// `reluctance` as the span widens, so a one-cell stream is nearly free, a few-cell
/// ford is cheap, and a wide deep river asymptotes to `reluctance` rather than a
/// linear wall — the bounded "elk swim rivers" model. Pure; tested below.
pub fn swim_cost(effective_cells: f32, reluctance: f32) -> f32 {
    reluctance * (1.0 - (-effective_cells.max(0.0) / SWIM_SCALE).exp())
}

/// Look across a water span for the far bank a crossing would aim at. Walks from
/// `cell` in `(dx, dy)` over contiguous water, accumulating the *ford-weighted*
/// water-cell count (a ford counts as `ford_discount` of a full cell), and returns
/// `(far_bank_cell, effective_cells)` at the first dry cell reached within
/// `max_peek` steps. `None` when the step does not enter water, the water never
/// ends within reach, or the path runs off the grid.
///
/// This is what lets a hungry herd "see" greener ground beyond a river it cannot
/// otherwise perceive (the far bank sits past the grass-gradient radius). The
/// caller prices the crossing with `swim_cost` and reads the far bank's
/// attractiveness, so the barrier becomes a decision instead of a wall.
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
            // Dry cell: the far bank — but only if we actually crossed water.
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

    // ── step_water_penalty ────────────────────────────────────────────────────

    // On a ford the penalty collapses to nearly zero regardless of water depth.
    #[test]
    fn ford_penalty_is_near_zero() {
        let cost = step_water_penalty(1.0, true, 2.0, 0.1);
        assert!(cost < 0.25, "ford should be nearly free, got {cost}");
    }

    // Off a ford the penalty is identical to today's plain water × cost.
    #[test]
    fn non_ford_penalty_equals_water_times_cost() {
        let cost = step_water_penalty(0.7, false, 2.0, 0.1);
        assert!((cost - 0.7 * 2.0).abs() < 1e-6);
    }

    // Metamorphic: penalty off a ford is monotone increasing in water level.
    #[test]
    fn penalty_monotone_in_water() {
        let low = step_water_penalty(0.2, false, 2.0, 0.1);
        let high = step_water_penalty(0.8, false, 2.0, 0.1);
        assert!(high > low);
    }

    // Deep non-ford water costs strictly more than a shallow tributary.
    #[test]
    fn deep_non_ford_beats_shallow_tributary() {
        let tributary = step_water_penalty(0.1, false, 2.0, 0.1); // shallow
        let deep = step_water_penalty(1.0, false, 2.0, 0.1);      // main channel
        assert!(deep > tributary);
    }

    // ── grass_gradient ────────────────────────────────────────────────────────

    fn flat_grid(w: usize, h: usize, forage: f32) -> super::super::super::grid::Grid {
        let mut g = crate::grid::Grid::new(w, h);
        for i in 0..g.len() {
            g.set_grass(i, forage);
        }
        g
    }

    // A fully uniform forage field has zero gradient — no direction to pull.
    // Grid is 21x21 so the center cell's full radius-5 neighbourhood is in bounds.
    #[test]
    fn gradient_zero_on_flat_forage() {
        let grid = flat_grid(21, 21, 0.5);
        let params = default_params();
        let center = 10 * 21 + 10;
        let g = grass_gradient(center, &grid, &params);
        assert!(g.length() < 1e-4, "flat forage must yield zero gradient, got {g:?}");
    }

    // The gradient points toward the richer cell (+x direction).
    #[test]
    fn gradient_points_toward_richer_forage() {
        let mut grid = crate::grid::Grid::new(21, 21);
        let center = 10 * 21 + 10;
        let right = center + 1; // col+1, same row
        grid.set_grass(right, 1.0);
        let params = default_params();
        let g = grass_gradient(center, &grid, &params);
        assert!(g.x > 0.0, "gradient must point toward richer forage (+x), got {g:?}");
        assert!(g.x.abs() > g.y.abs(), "gradient must be predominantly rightward");
    }

    // Raising a neighbour's forage raises the gradient magnitude (metamorphic).
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

    // With freshness_weight > 0, a high-freshness neighbour attracts even when a
    // higher-biomass neighbour sits the other way (fresh beats mature).
    #[test]
    fn gradient_points_toward_fresh_over_mature_biomass() {
        let mut grid = crate::grid::Grid::new(21, 21);
        let center = 10 * 21 + 10;
        let left = center - 1;  // col-1: high biomass, stale
        let right = center + 1; // col+1: lower biomass, but fresh

        // Left neighbour: high forage, zero freshness.
        grid.set_grass(left, 0.9);
        // Right neighbour: moderate forage but high freshness.
        grid.set_grass(right, 0.3);
        // Set freshness on right directly (public field).
        grid.freshness[right] = 2.0;

        let mut params = default_params();
        params.freshness_weight = 1.5;

        let g = grass_gradient(center, &grid, &params);
        assert!(
            g.x > 0.0,
            "with freshness_weight > 0, fresh cell (+x) must beat mature (-x): gradient={g:?}"
        );
    }

    // With freshness_weight == 0, gradient is identical to today's biomass-only result.
    #[test]
    fn gradient_freshness_weight_zero_is_identity() {
        let mut grid = crate::grid::Grid::new(21, 21);
        let center = 10 * 21 + 10;
        let right = center + 1;
        grid.set_grass(right, 0.8);
        grid.freshness[right] = 5.0; // high freshness but weight is 0

        let mut params_zero = default_params();
        params_zero.freshness_weight = 0.0;

        let mut params_positive = default_params();
        params_positive.freshness_weight = 0.0; // also zero, so identical

        let g_zero = grass_gradient(center, &grid, &params_zero);
        let g_pos = grass_gradient(center, &grid, &params_positive);

        assert!(
            (g_zero - g_pos).length() < 1e-6,
            "freshness_weight=0 must yield identical result regardless of freshness field"
        );
    }

    // ── forage_sightline ──────────────────────────────────────────────────────

    // Off by default: weight 0 ⇒ zero pull no matter what lies ahead (identity —
    // the grass drive stays the local gradient).
    #[test]
    fn sightline_off_by_default_is_zero() {
        let mut grid = crate::grid::Grid::new(40, 5);
        let center = 2 * 40 + 5;
        grid.set_grass(center + 15, 1.0); // rich patch far east
        let params = default_params(); // sightline_weight 0, range 0
        assert_eq!(forage_sightline(center, &grid, &params), Vec2::ZERO);
    }

    // The breaking input: rich forage *beyond* grass_radius to the east, none
    // underfoot. The local gradient can't see it; the sightline must pull +x.
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

    // No pull to nowhere: when nothing ahead beats underfoot the sightline is zero,
    // so a fed elk on good grass is never yanked east off it.
    #[test]
    fn sightline_zero_when_nothing_better_ahead() {
        let mut grid = crate::grid::Grid::new(40, 5);
        let center = 2 * 40 + 5;
        grid.set_grass(center, 1.0); // rich underfoot, barren ahead
        let mut params = default_params();
        params.sightline_range = 16.0;
        params.sightline_weight = 1.0;
        assert_eq!(forage_sightline(center, &grid, &params), Vec2::ZERO);
    }

    // Freshness is the long-range beacon: a fresh-but-not-yet-biomassy front ahead
    // pulls east once freshness_weight makes it the richer attractiveness.
    #[test]
    fn sightline_follows_the_freshness_front() {
        let mut grid = crate::grid::Grid::new(40, 5);
        let center = 2 * 40 + 5;
        let front = center + 10;
        grid.freshness[front] = 3.0; // green-up front, low standing crop
        let mut params = default_params();
        params.sightline_range = 16.0;
        params.sightline_weight = 1.0;
        params.freshness_weight = 1.5;
        assert!(forage_sightline(center, &grid, &params).x > 0.0);
    }

    // ── cross_desire ─────────────────────────────────────────────────────────

    // Example scenario — a lead herd: fresh grass both ahead (+x) and across (+y).
    // Advancing along +x is as good as crossing and costs nothing, so it stays.
    #[test]
    fn lead_herd_with_forage_ahead_does_not_cross() {
        assert!(cross_desire(0.5, 0.9, 0.9, 0.2) <= 0.0);
    }

    // Example scenario — a trailing herd: the lead ate the +x corridor, so `here`
    // and `ahead` are bare; only the far bank is green. It crosses.
    #[test]
    fn trailing_herd_crosses_for_the_far_bank() {
        assert!(cross_desire(0.1, 0.1, 0.9, 0.2) > 0.0);
    }

    // Metamorphic relation — richer forage AHEAD can only lower the urge to cross:
    // a better dry option competes with the far bank. (Monotone ↓ in `ahead`.)
    #[test]
    fn more_forage_ahead_never_raises_cross_desire() {
        assert!(cross_desire(0.1, 0.8, 0.9, 0.2) <= cross_desire(0.1, 0.2, 0.9, 0.2));
    }

    // Metamorphic relation — richer forage ACROSS can only raise it. (Monotone ↑.)
    #[test]
    fn more_forage_across_never_lowers_cross_desire() {
        assert!(cross_desire(0.1, 0.1, 0.9, 0.2) >= cross_desire(0.1, 0.1, 0.4, 0.2));
    }

    // Metamorphic relation — a costlier ford can only lower it. (Monotone ↓ in cost.)
    #[test]
    fn costlier_crossing_never_raises_cross_desire() {
        assert!(cross_desire(0.1, 0.1, 0.9, 0.5) <= cross_desire(0.1, 0.1, 0.9, 0.1));
    }

    // ── forage_across ─────────────────────────────────────────────────────────

    /// A row world: dry start (0), two water cells (1,2), dry far bank (3) with grass.
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
        // Two non-ford water cells ⇒ effective_cells 2.0; far bank is cell 3 (forage 0.5).
        let (far_cell, cells) = forage_across(&grid, 0, 1, 0, 0.1, 12).unwrap();
        assert_eq!(far_cell, 3);
        assert!((cells - 2.0).abs() < 1e-6);
    }

    #[test]
    fn forage_across_is_none_when_step_is_dry() {
        // Stepping the other way (−x off cell 0) and into dry land yields no crossing.
        let mut grid = Grid::new(6, 1);
        grid.set_grass(1, 0.5); // dry neighbour to the +x side
        assert!(forage_across(&grid, 0, 1, 0, 0.1, 12).is_none());
    }

    #[test]
    fn forage_across_is_none_when_far_bank_out_of_reach() {
        let mut grid = Grid::new(6, 1);
        for i in 1..6 {
            grid.set_water(i, 1.0); // water all the way to the edge — no far bank
        }
        assert!(forage_across(&grid, 0, 1, 0, 0.1, 12).is_none());
    }

    #[test]
    fn forage_across_counts_fords_as_fractional_cells() {
        let mut grid = river_row();
        grid.set_ford(1, true);
        grid.set_ford(2, true);
        // ford_discount 0.1 ⇒ each ford cell counts as 0.1 of a full cell ⇒ 0.2 total.
        let (_, cells) = forage_across(&grid, 0, 1, 0, 0.1, 12).unwrap();
        assert!((cells - 0.2).abs() < 1e-6, "ford span should count 2×0.1 = 0.2, got {cells}");
    }

    // ── swim_cost ───────────────────────────────────────────────────────────────

    // A zero-width span costs nothing; the deterrent only exists where there is water.
    #[test]
    fn swim_cost_is_zero_for_no_water() {
        assert!(swim_cost(0.0, 0.6).abs() < 1e-6);
    }

    // The cost saturates: it never exceeds the reluctance ceiling, however wide.
    #[test]
    fn swim_cost_is_bounded_by_reluctance() {
        for cells in [1.0_f32, 4.0, 12.0, 50.0, 1000.0] {
            let c = swim_cost(cells, 0.6);
            assert!(c < 0.6 + 1e-6, "cost {c} exceeds reluctance at {cells} cells");
        }
        // A very wide river is within a whisker of the ceiling.
        assert!(swim_cost(1000.0, 0.6) > 0.6 - 1e-3);
    }

    // Monotone increasing in width: a wider span never costs less. Breaking input:
    // if the sign flipped, a wide river would read as cheaper than a stream.
    #[test]
    fn swim_cost_rises_with_width() {
        let widths = [0.0_f32, 1.0, 2.0, 4.0, 8.0, 16.0];
        let costs: Vec<f32> = widths.iter().map(|&w| swim_cost(w, 0.6)).collect();
        for w in costs.windows(2) {
            assert!(w[1] >= w[0], "cost must not fall as width rises: {} < {}", w[1], w[0]);
        }
    }

    // A few-cell ford is far cheaper to decide on than a wide deep river — the
    // property that makes a herd prefer the shallows. (Same reluctance both sides.)
    #[test]
    fn swim_cost_makes_a_narrow_ford_cheaper_than_a_wide_river() {
        let ford = swim_cost(0.4, 0.6); // 4 ford cells at discount 0.1
        let river = swim_cost(14.0, 0.6); // a wide deep channel
        assert!(ford < river * 0.5, "a ford ({ford}) should be much cheaper than a river ({river})");
    }
}
