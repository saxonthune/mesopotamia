use bevy::prelude::*;

use crate::events::{Event, EventKind, EventLog};
use crate::grid::Grid;

use super::components::{Elk, ElkParams, Herds, Spawner, BITE, CHEW_TICKS, ENERGY_DRAIN, GRAZE_FLOOR};
use super::herding::Herding;
use super::ledger::EnergyFlows;

/// The bite an elk takes from a grass cell, or `None` when the cell isn't worth
/// biting. Grazing leaves a giving-up density — `floor_frac · capacity` of grass
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
fn worthwhile_bite(grid: &Grid, cell: usize) -> Option<f32> {
    grass_bite(grid.grass(cell), grid.capacity(cell), BITE, GRAZE_FLOOR)
}

pub(super) fn graze(
    mut grid: ResMut<Grid>,
    params: Res<ElkParams>,
    mut elk: Query<(&mut Elk, &mut Herding)>,
    mut flows: ResMut<EnergyFlows>,
) {
    for (mut elk, mut herd) in &mut elk {
        // Still chewing the last mouthful: digest a tick, take no new bite. The herd
        // is pinned in place meanwhile (the Graze steer reads `chew`), so a bite is
        // one mouthful every `CHEW_TICKS + 1` ticks rather than a continuous nibble.
        if herd.chew > 0 {
            herd.chew -= 1;
            elk.grazing = true;
            continue;
        }
        // Feeding is decoupled from movement: an elk eats whenever worthwhile food
        // sits underfoot and the state permits it (suppressed only mid-crossing) —
        // the structural guarantee (doc03.01.09) that the movement model can never
        // starve a herd standing on food. The shrub/grass checks below gate on
        // availability, so this only *permits* feeding; it never forces an empty bite.
        if herd.permits_feeding() {
            // Grass and shrub are one food: a bite of either pays the same
            // `graze_yield`. Shrub is drawn first where present (it stands on dry
            // ground a grass cell can't), then grass under the giving-up density.
            let before = elk.energy;
            if grid.shrubs(elk.cell) > GRAZE_FLOOR * grid.shrub_cap(elk.cell) {
                let bitten = BITE.min(grid.shrubs(elk.cell));
                grid.eat_shrubs(elk.cell, bitten);
                elk.energy = (elk.energy + bitten * params.graze_yield).min(1.0);
                elk.grazing = true;
                herd.chew = CHEW_TICKS;
            } else if let Some(bitten) = worthwhile_bite(&grid, elk.cell) {
                grid.grow_grass(elk.cell, -bitten);
                elk.energy = (elk.energy + bitten * params.graze_yield).min(1.0);
                elk.grazing = true;
                herd.chew = CHEW_TICKS;
            } else {
                elk.grazing = false;
            }
            flows.intake += elk.energy - before;
        } else {
            // Mid-crossing: no eating this tick.
            elk.grazing = false;
        }
    }
}

/// Every tick the elk spends energy just being alive; an elk that runs out
/// starves and despawns — the consequence that drives the herd to keep feeding.
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
