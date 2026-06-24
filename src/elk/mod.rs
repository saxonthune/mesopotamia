mod components;
mod color;
mod ledger;
mod spawn;
mod movement;
mod metabolism;
mod score;
pub mod abundance;
pub mod presets;
pub mod ratios;

pub use components::{Cohort, DriveSample, DriveSamples, Elk, ElkParams, HabitatIntake, Herds, LastDecision, Packs, Spawner};
pub use ratios::RatioControls;
pub use score::Score;
// Used by sim_harness probe infrastructure; not imported by the main binary.
#[allow(unused_imports)]
pub use components::ProbeSeed;
#[allow(unused_imports)]
pub use ledger::{energy_expected_delta, energy_ledger_closes, population_balances, EnergyFlows};
#[allow(unused_imports)]
pub use movement::{cell_water_penalty, combine_drives, cross_desire, forage_gate, grass_gradient, graze_value, migration_residual, stand_value, step_water_penalty, Act, Candidate, Decomposable, Decision, Drives};
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
            .init_resource::<HabitatIntake>()
            .init_resource::<abundance::AbundanceParams>()
            .init_resource::<RatioControls>()
            .init_resource::<Score>()
            .add_systems(Update, (spawn::tally_herds, ratios::apply_ratios))
            .add_systems(OnExit(Sim::Running), spawn::teardown)
            .add_systems(
                FixedUpdate,
                (
                    movement::herd_move,
                    metabolism::graze,
                    metabolism::metabolize,
                    metabolism::migrate_pressure,
                    spawn::spawn_waves,
                    spawn::cull,
                    // Order-independent: the monotonic event cursor folds each
                    // despawn exactly once, at worst a tick after it happens.
                    score::update_score,
                ).run_if(in_state(Sim::Running)),
            );
    }
}
