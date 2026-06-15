use bevy::prelude::*;

use crate::grid::Grid;

use super::components::{Elk, ElkParams, Herds, Packs};

const POOP_PER_GRAZE: f32 = 0.3;
const DIGEST_TICKS: u32 = 20;

pub(super) fn migrate_pressure(params: Res<ElkParams>, mut packs: ResMut<Packs>) {
    for i in 0..packs.migration.len() {
        let g = packs.growth[i] * params.mig_growth;
        packs.migration[i] = (packs.migration[i] + g).min(1.0);
    }
}

pub(super) fn graze(mut grid: ResMut<Grid>, params: Res<ElkParams>, mut elk: Query<&mut Elk>) {
    for mut elk in &mut elk {
        if grid.browse(elk.cell) > 0.05 {
            // Browse first: a big, concentrated bite that strips the shrub and
            // pays more energy than grass — the reward for crossing dry ground.
            grid.eat_browse(elk.cell, params.browse_bite);
            elk.energy = (elk.energy + params.browse_energy).min(1.0);
            elk.digesting.push(DIGEST_TICKS);
            elk.grazing = true;
        } else if grid.grass(elk.cell) > 0.0 {
            grid.grow_grass(elk.cell, -params.bite);
            elk.energy = (elk.energy + params.energy_per_bite).min(1.0);
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
    mut elk: Query<(Entity, &mut Elk)>,
) {
    for (entity, mut elk) in &mut elk {
        elk.energy -= params.energy_drain;
        if elk.energy <= 0.0 {
            if let Some(c) = herds.cohorts.get_mut(&elk.code) {
                c.deaths += 1;
            }
            commands.entity(entity).despawn();
        }
    }
}
