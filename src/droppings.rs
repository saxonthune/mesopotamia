//! The droppings nutrient cycle — a self-contained, omittable feature plugin.
//!
//! Elk that graze schedule a digestion timer; when it expires they deposit poop
//! at their current cell, and `fertilize` converts that poop back into grass.
//! This closes the grass → elk → poop → grass loop.
//!
//! It is **simulation-only** (no rendering dependency) so it loads headless in
//! `sim_harness.rs`. The poop *storage* lives on `Grid` (`poop`, `add_poop`,
//! `MAX_POOP`) and poop *rendering* lives in `RenderPlugin`; only the systems
//! that drive the cycle live here. Omitting `DroppingsPlugin` from a binary's
//! `add_plugins` tuple disables the whole cycle — no poop is produced, so the
//! render dots stay inert.

use bevy::prelude::*;

use crate::elk::Elk;
use crate::grid::Grid;
use crate::sim::Sim;

/// Poop deposited at a cell when one digestion timer expires.
const POOP_PER_GRAZE: f32 = 0.3;
/// Ticks between a grazing bite and the poop it eventually drops.
const DIGEST_TICKS: u32 = 20;

/// Controls the poop → grass conversion that runs every tick.
#[derive(Resource)]
pub struct Fertility {
    /// Poop consumed per cell per tick.
    pub rate: f32,
    /// Fraction of consumed poop that becomes grass; the rest (1 - efficiency) is lost.
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

/// Schedules and resolves digestion. An elk that grazed this tick (the
/// `elk.grazing` beacon `graze` raises) starts a fresh `DIGEST_TICKS` timer;
/// every pending timer counts down, and an expired one drops poop at the elk's
/// *current* cell — so nutrients move with the body, not back to the eat-site.
///
/// Reading `grazing` rather than owning the push keeps poop knowledge out of
/// `graze`. The decoupling allows a 1-tick scheduling jitter under ambiguous
/// ordering, immaterial against the `DIGEST_TICKS` delay.
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
