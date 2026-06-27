use bevy::prelude::*;

use crate::events::{Event, EventKind, EventLog};
use crate::grid::Grid;

use super::components::{
    Elk, ElkParams, Herds, Spawner, BITE, CHEW_TICKS, ENERGY_DRAIN, GRAZE_FLOOR, GRAZE_HALF_SAT,
};
use super::herding::Herding;
use super::ledger::EnergyFlows;

/// Smoothing for the per-elk intake EMA (~10-tick memory: ~2 bite+chew cycles).
const INTAKE_ALPHA: f32 = 0.1;

/// Holling Type-II intake, normalized to a full bite on full forage: fresh forage (`fullness` 1)
/// pays a full bite, a grazed-down cell pays proportionally less, so a trailing elk must dwell
/// longer for the same energy — the front/back intake gradient that lets a rolling front emerge.
fn functional_response(fullness: f32, half_sat: f32) -> f32 {
    let f = fullness.clamp(0.0, 1.0);
    f * (1.0 + half_sat) / (f + half_sat)
}

/// Density-dependent bite above the giving-up floor, or `None` — `None` (not zero) is what steers
/// elk off a grazed-down patch. The bite shrinks with fullness via the functional response.
fn grass_bite(amount: f32, capacity: f32, bite: f32, floor_frac: f32, half_sat: f32) -> Option<f32> {
    if capacity <= 0.0 {
        return None;
    }
    let available = amount - floor_frac * capacity;
    if available <= 0.0 {
        return None;
    }
    let bitten = (bite * functional_response(amount / capacity, half_sat)).min(available);
    if bitten > 0.0 { Some(bitten) } else { None }
}

pub(super) fn graze(
    mut grid: ResMut<Grid>,
    params: Res<ElkParams>,
    mut elk: Query<(&mut Elk, &mut Herding)>,
    mut flows: ResMut<EnergyFlows>,
) {
    for (mut elk, mut herd) in &mut elk {
        let before = elk.energy;
        // Feeding is decoupled from movement (doc03.01.09): movement can never starve a herd on food.
        if herd.chew > 0 {
            herd.chew -= 1;
            elk.grazing = true;
        } else if herd.permits_feeding() {
            // Shrub first (dry ground); then grass. Both bite per the density-dependent response.
            let cell = elk.cell;
            if let Some(bitten) =
                grass_bite(grid.shrubs(cell), grid.shrub_cap(cell), BITE, GRAZE_FLOOR, GRAZE_HALF_SAT)
            {
                grid.eat_shrubs(cell, bitten);
                elk.energy = (elk.energy + bitten * params.graze_yield).min(1.0);
                elk.grazing = true;
                herd.chew = CHEW_TICKS;
            } else if let Some(bitten) =
                grass_bite(grid.grass(cell), grid.capacity(cell), BITE, GRAZE_FLOOR, GRAZE_HALF_SAT)
            {
                grid.grow_grass(cell, -bitten);
                elk.energy = (elk.energy + bitten * params.graze_yield).min(1.0);
                elk.grazing = true;
                herd.chew = CHEW_TICKS;
            } else {
                elk.grazing = false;
            }
        } else {
            elk.grazing = false;
        }
        let gained = elk.energy - before;
        flows.intake += gained;
        // EMA over the whole bite+chew cycle → the elk's true per-tick intake rate, the leave signal.
        herd.intake_ema += INTAKE_ALPHA * (gained - herd.intake_ema);
    }
}

pub(super) fn metabolize(
    mut commands: Commands,
    mut herds: ResMut<Herds>,
    spawner: Res<Spawner>,
    mut event_log: ResMut<EventLog>,
    mut elk: Query<(Entity, &mut Elk)>,
    mut flows: ResMut<EnergyFlows>,
) {
    let w = crate::grid::GRID_WIDTH as isize;
    for (entity, mut elk) in &mut elk {
        flows.drain += ENERGY_DRAIN;
        elk.energy -= ENERGY_DRAIN;
        if elk.energy <= 0.0 {
            flows.deaths_energy += elk.energy;
            if let Some(c) = herds.cohorts.get_mut(&elk.code) {
                c.deaths += 1;
            }
            // Last step direction, so the death-sites overlay can show which way it was heading.
            let (pc, c) = (elk.prev_cell as isize, elk.cell as isize);
            let (dx, dy) = (c % w - pc % w, c / w - pc / w);
            let chosen_step = if dx == 0 && dy == 0 { None } else { Some((dx, dy)) };
            event_log.push(Event {
                tick: spawner.elapsed as u64,
                cell: elk.cell,
                kind: EventKind::Starved,
                energy: elk.energy,
                chosen_step,
            });
            commands.entity(entity).despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{functional_response, grass_bite};

    const HS: f32 = 0.2;

    fn close(got: Option<f32>, want: f32) -> bool {
        matches!(got, Some(b) if (b - want).abs() < 1e-5)
    }

    #[test]
    fn fresh_cell_gives_a_full_bite() {
        // full forage → response 1.0 → the bite is unchanged from the pre-response model.
        assert!(close(grass_bite(1.0, 1.0, 0.5, 0.3, HS), 0.5));
    }

    #[test]
    fn bite_is_capped_by_what_sits_above_the_floor() {
        // grass 0.4, floor 0.3·1.0 = 0.3 → only 0.1 is edible, less than the response-scaled bite.
        assert!(close(grass_bite(0.4, 1.0, 0.5, 0.3, HS), 0.1));
    }

    #[test]
    fn grass_at_the_floor_is_not_worth_biting() {
        assert_eq!(grass_bite(0.3, 1.0, 0.5, 0.3, HS), None);
        assert_eq!(grass_bite(0.2, 1.0, 0.5, 0.3, HS), None);
    }

    #[test]
    fn floor_scales_with_capacity() {
        // cap 0.4, floor 0.3·0.4 = 0.12; grass 0.15 → only ~0.03 edible (response-scaled bite is larger).
        assert!(close(grass_bite(0.15, 0.4, 0.5, 0.3, HS), 0.03));
        // same grass on a cell whose floor exceeds it → nothing.
        assert_eq!(grass_bite(0.15, 1.0, 0.5, 0.3, HS), None);
    }

    #[test]
    fn depleted_forage_yields_a_smaller_bite() {
        // No floor binding, so the functional response alone shrinks the bite as forage thins —
        // the front/back gradient: a trailing elk on grazed ground gets less per bite.
        let full = grass_bite(1.0, 1.0, 0.1, 0.0, HS).unwrap();
        let half = grass_bite(0.5, 1.0, 0.1, 0.0, HS).unwrap();
        let low = grass_bite(0.1, 1.0, 0.1, 0.0, HS).unwrap();
        assert!((full - 0.1).abs() < 1e-6, "full forage pays a full bite: {full}");
        assert!(half < full, "half-grazed pays less: {half} !< {full}");
        assert!(low < half, "near-bare pays least: {low} !< {half}");
    }

    #[test]
    fn functional_response_normalizes_to_one_on_full_forage() {
        assert!((functional_response(1.0, HS) - 1.0).abs() < 1e-6);
        assert_eq!(functional_response(0.0, HS), 0.0);
        assert!(
            functional_response(0.5, HS) > functional_response(0.25, HS),
            "response is monotone in fullness"
        );
        assert!(functional_response(0.5, HS) < 1.0, "below full forage the response is < 1");
    }
}
