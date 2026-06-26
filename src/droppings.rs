//! Droppings nutrient cycle: grass → elk → poop → grass. Simulation-only (no render dep).
//! Poop storage on `Grid`; rendering in `RenderPlugin`. Omit `DroppingsPlugin` to disable the whole cycle.

use bevy::prelude::*;

use crate::elk::Elk;
use crate::grid::Grid;
use crate::sim::Sim;

const POOP_PER_GRAZE: f32 = 0.3;
const DIGEST_TICKS: u32 = 20;

#[derive(Resource)]
pub struct Fertility {
    pub rate: f32,
    pub efficiency: f32,
}

impl Default for Fertility {
    fn default() -> Self {
        Self { rate: 0.05, efficiency: 0.5 }
    }
}

pub struct DroppingsPlugin;

impl Plugin for DroppingsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Fertility>()
            .add_systems(FixedUpdate, (digest, fertilize).run_if(in_state(Sim::Running)));
    }
}

/// Reads `elk.grazing` (set by `graze`) to stay decoupled; poop drops at *current* cell, not the eat-site.
fn digest(mut grid: ResMut<Grid>, mut elk: Query<&mut Elk>) {
    for mut elk in &mut elk {
        if elk.grazing {
            elk.digesting.push(DIGEST_TICKS);
        }
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

fn fertilize(mut grid: ResMut<Grid>, fertility: Res<Fertility>) {
    for index in 0..grid.len() {
        let consumed = grid.poop(index).min(fertility.rate);
        if consumed <= 0.0 {
            continue;
        }
        grid.add_poop(index, -consumed);
        grid.grow_grass(index, consumed * fertility.efficiency);
    }
}
