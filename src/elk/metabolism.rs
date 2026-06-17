use bevy::prelude::*;

use crate::events::{Event, EventKind, EventLog};
use crate::grid::Grid;

use super::components::{Elk, ElkParams, Herds, LastDecision, Packs, Spawner};
use super::ledger::EnergyFlows;

const POOP_PER_GRAZE: f32 = 0.3;
const DIGEST_TICKS: u32 = 20;

pub(super) fn migrate_pressure(params: Res<ElkParams>, mut packs: ResMut<Packs>) {
    for i in 0..packs.migration.len() {
        let g = packs.growth[i] * params.mig_growth;
        packs.migration[i] = (packs.migration[i] + g).min(1.0);
    }
}

/// The bite an elk takes from a grass cell, or `None` when the cell isn't worth
/// biting. Grazing leaves a giving-up density — `graze_floor · capacity` of grass
/// stays in the ground — and a bite takes only what sits above that floor, capped
/// at `bite`. Returning `None` (rather than a zero bite) is what stops an elk from
/// camping a grazed-down patch: below the floor there is nothing to eat, so the
/// move drives steer it elsewhere.
fn grass_bite(grass: f32, capacity: f32, bite: f32, floor_frac: f32) -> Option<f32> {
    let available = grass - floor_frac * capacity;
    let bitten = bite.min(available);
    if bitten > 0.0 { Some(bitten) } else { None }
}

/// Convenience wrapper reading the cell straight off the grid.
fn worthwhile_bite(grid: &Grid, cell: usize, params: &ElkParams) -> Option<f32> {
    grass_bite(grid.grass(cell), grid.capacity(cell), params.bite, params.graze_floor)
}

pub(super) fn graze(
    mut grid: ResMut<Grid>,
    params: Res<ElkParams>,
    mut elk: Query<&mut Elk>,
    mut flows: ResMut<EnergyFlows>,
) {
    for mut elk in &mut elk {
        if grid.shrubs(elk.cell) > 0.05 {
            // Shrubs first: a big, concentrated bite that strips the shrub and
            // pays more energy than grass — the reward for crossing dry ground.
            grid.eat_shrubs(elk.cell, params.shrub_bite);
            let before = elk.energy;
            elk.energy = (elk.energy + params.shrub_energy).min(1.0);
            flows.intake += elk.energy - before;
            elk.digesting.push(DIGEST_TICKS);
            elk.grazing = true;
        } else if let Some(bitten) = worthwhile_bite(&grid, elk.cell, &params) {
            // Giving-up density: a bite takes only the grass above the floor and
            // pays energy in proportion. A grazed-down or thin cell yields nothing
            // worth biting, so the elk moves on instead of camping a patch and
            // sipping its regrowth forever.
            grid.grow_grass(elk.cell, -bitten);
            let before = elk.energy;
            elk.energy = (elk.energy + bitten * params.graze_yield).min(1.0);
            flows.intake += elk.energy - before;
            elk.digesting.push(DIGEST_TICKS);
            elk.grazing = true; // raise the beacon other elk forage toward
        } else {
            elk.grazing = false;
        }
    }
}

/// Counts down each pending meal; when one expires, drop poop at the elk's
/// *current* cell — so nutrients move with the body, not back to the eat-site.
pub(super) fn digest(mut grid: ResMut<Grid>, mut elk: Query<&mut Elk>) {
    for mut elk in &mut elk {
        let cell = elk.cell;
        elk.digesting.retain_mut(|t| {
            if *t == 0 {
                grid.add_poop(cell, POOP_PER_GRAZE);
                false
            } else {
                *t -= 1;
                true
            }
        });
    }
}

/// Every tick the elk spends energy just being alive; an elk that runs out
/// starves and despawns — the consequence that drives the herd to keep feeding.
pub(super) fn metabolize(
    mut commands: Commands,
    params: Res<ElkParams>,
    mut herds: ResMut<Herds>,
    spawner: Res<Spawner>,
    mut event_log: ResMut<EventLog>,
    mut elk: Query<(Entity, &mut Elk, Option<&LastDecision>)>,
    mut flows: ResMut<EnergyFlows>,
) {
    for (entity, mut elk, last_decision) in &mut elk {
        flows.drain += params.energy_drain;
        elk.energy -= params.energy_drain;
        if elk.energy <= 0.0 {
            flows.deaths_energy += elk.energy;
            if let Some(c) = herds.cohorts.get_mut(&elk.code) {
                c.deaths += 1;
            }
            let chosen_step = last_decision
                .and_then(|ld| ld.0.chosen.and_then(|i| ld.0.options.get(i).map(|e| e.step)));
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
    use super::grass_bite;

    fn close(got: Option<f32>, want: f32) -> bool {
        matches!(got, Some(b) if (b - want).abs() < 1e-5)
    }

    // A fresh cell pays a full bite: plenty of grass above the floor.
    #[test]
    fn fresh_cell_gives_a_full_bite() {
        assert!(close(grass_bite(1.0, 1.0, 0.5, 0.3), 0.5));
    }

    // The bite never reaches below the floor — it takes only grass-above-floor.
    #[test]
    fn bite_is_capped_by_what_sits_above_the_floor() {
        // grass 0.4, floor 0.3·1.0 = 0.3 → only ~0.1 is edible, less than `bite`.
        assert!(close(grass_bite(0.4, 1.0, 0.5, 0.3), 0.1));
    }

    // At or below the floor the cell isn't worth biting — the camp exploit dies here.
    #[test]
    fn grass_at_the_floor_is_not_worth_biting() {
        assert_eq!(grass_bite(0.3, 1.0, 0.5, 0.3), None);
        assert_eq!(grass_bite(0.2, 1.0, 0.5, 0.3), None);
    }

    // The floor scales with capacity: a thin (low-capacity) cell gives up sooner in
    // absolute terms, so marginal land isn't worth lingering on.
    #[test]
    fn floor_scales_with_capacity() {
        // cap 0.4, floor 0.3·0.4 = 0.12; grass 0.15 → only ~0.03 edible.
        assert!(close(grass_bite(0.15, 0.4, 0.5, 0.3), 0.03));
        // same grass on a cell whose floor exceeds it → nothing.
        assert_eq!(grass_bite(0.15, 1.0, 0.5, 0.3), None);
    }
}
