mod components;
mod color;
mod ledger;
mod spawn;
mod movement;
mod metabolism;
pub mod abundance;

pub use components::{Cohort, DriveSample, DriveSamples, Elk, ElkParams, Herds, LastDecision, Packs, Spawner};
// Used by sim_harness probe infrastructure; not imported by the main binary.
#[allow(unused_imports)]
pub use components::ProbeSeed;
#[allow(unused_imports)]
pub use ledger::{energy_expected_delta, energy_ledger_closes, population_balances, EnergyFlows};
#[allow(unused_imports)]
pub use movement::{cell_water_penalty, combine_drives, cross_desire, grass_gradient, migration_residual, step_water_penalty, Decomposable, Decision, Drives, StepEval};
// Lifecycle constants the macro-sim harness asserts against — exported so the
// tests read the single source of truth instead of mirroring magic numbers.
#[allow(unused_imports)]
pub use spawn::{EDGE_COL, TARGET_POPULATION};
pub(crate) use color::elk_color;

use bevy::prelude::*;

use crate::events::EventsPlugin;
use crate::sim::Sim;

pub struct ElkSimPlugin;

pub const PACK_COUNT: usize = 8;

impl Plugin for ElkSimPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(EventsPlugin)
            .insert_resource(Packs::new())
            .insert_resource(DriveSamples {
                per_slot: vec![DriveSample::default(); PACK_COUNT],
            })
            .init_resource::<Spawner>()
            .init_resource::<ElkParams>()
            .init_resource::<Herds>()
            .init_resource::<EnergyFlows>()
            .init_resource::<abundance::AbundanceParams>()
            .add_systems(Update, spawn::tally_herds)
            .add_systems(OnExit(Sim::Running), spawn::teardown)
            .add_systems(
                FixedUpdate,
                (
                    movement::herd_move,
                    metabolism::graze,
                    metabolism::digest,
                    metabolism::metabolize,
                    metabolism::migrate_pressure,
                    spawn::spawn_waves,
                    spawn::cull,
                ).run_if(in_state(Sim::Running)),
            );
    }
}
