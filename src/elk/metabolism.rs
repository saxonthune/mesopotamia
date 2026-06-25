use bevy::prelude::*;

use crate::events::{Event, EventKind, EventLog};
use crate::grid::Grid;

use super::components::{Elk, ElkParams, Herds, Packs, Spawner};
use super::herding::{HerdState, Herding};
use super::ledger::EnergyFlows;

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
    mut elk: Query<(&mut Elk, &Herding)>,
    mut flows: ResMut<EnergyFlows>,
) {
    let alpha = params.intake_smoothing;

    for (mut elk, herd) in &mut elk {
        let intake_this_tick;
        // Feeding is decoupled from movement: an elk eats whenever worthwhile food
        // sits underfoot and it is not mid-water-crossing — the structural guarantee
        // (doc03.01.09) that the movement model can never starve a herd standing on
        // food. The shrub/grass checks below gate on availability, so this only
        // *permits* feeding; it never forces an empty bite.
        if herd.state != HerdState::Cross {
            if grid.shrubs(elk.cell) > 0.05 {
                // Shrubs first: a big, concentrated bite that strips the shrub and
                // pays more energy than grass — the reward for crossing dry ground.
                grid.eat_shrubs(elk.cell, params.shrub_bite);
                let before = elk.energy;
                elk.energy = (elk.energy + params.shrub_energy).min(1.0);
                intake_this_tick = elk.energy - before;
                flows.intake += intake_this_tick;
                elk.grazing = true;
            } else if let Some(bitten) = worthwhile_bite(&grid, elk.cell, &params) {
                // Giving-up density: a bite takes only the grass above the floor and
                // pays energy in proportion. A grazed-down or thin cell yields nothing
                // worth biting, so the elk moves on instead of camping a patch and
                // sipping its regrowth forever.
                grid.grow_grass(elk.cell, -bitten);
                let before = elk.energy;
                elk.energy = (elk.energy + bitten * params.graze_yield).min(1.0);
                intake_this_tick = elk.energy - before;
                flows.intake += intake_this_tick;
                elk.grazing = true; // raise the beacon other elk forage toward
            } else {
                intake_this_tick = 0.0;
                elk.grazing = false;
            }
        } else {
            // Mid-crossing: no eating this tick.
            intake_this_tick = 0.0;
            elk.grazing = false;
        }
        elk.intake_rate = (1.0 - alpha) * elk.intake_rate + alpha * intake_this_tick;
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
    mut elk: Query<(Entity, &mut Elk)>,
    mut flows: ResMut<EnergyFlows>,
) {
    let w = crate::grid::GRID_WIDTH as isize;
    for (entity, mut elk) in &mut elk {
        flows.drain += params.energy_drain;
        elk.energy -= params.energy_drain;
        if elk.energy <= 0.0 {
            flows.deaths_energy += elk.energy;
            if let Some(c) = herds.cohorts.get_mut(&elk.code) {
                c.deaths += 1;
            }
            // The elk's last grid step (prev_cell → cell), recorded on the death
            // event so the death-sites overlay can show which way it was heading.
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
