mod components;
mod color;
mod ledger;
mod spawn;
mod movement;
mod herding;
mod metabolism;
mod score;
pub mod abundance;
pub mod presets;
pub mod ratios;

pub use components::{Cohort, Elk, ElkParams, Herds, Spawner, CHEW_TICKS, ENERGY_DRAIN};
pub use ratios::RatioControls;
pub use score::Score;
#[allow(unused_imports)]
pub use ledger::{energy_expected_delta, energy_ledger_closes, population_balances, EnergyFlows};
#[allow(unused_imports)]
pub use movement::{cell_water_penalty, cross_desire, forage_across, forage_sightline, grass_gradient, step_water_penalty, swim_cost};
#[allow(unused_imports)]
pub use herding::{HerdParams, HerdState, Herding, Goal};
// Exported so harness tests read the single source of truth, not mirror magic numbers.
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
            .init_resource::<Spawner>()
            .init_resource::<ElkParams>()
            .init_resource::<HerdParams>()
            .init_resource::<Herds>()
            .init_resource::<EnergyFlows>()
            .init_resource::<abundance::AbundanceParams>()
            .init_resource::<RatioControls>()
            .init_resource::<Score>()
            .add_systems(Update, (spawn::tally_herds, ratios::apply_ratios))
            .add_systems(OnExit(Sim::Running), spawn::teardown)
            .add_systems(
                FixedUpdate,
                (
                    herding::herd_step,
                    metabolism::graze,
                    metabolism::metabolize,
                    spawn::spawn_waves,
                    spawn::cull,
                    // Order-independent: the monotonic event cursor folds each
                    // despawn exactly once, at worst a tick after it happens.
                    score::update_score,
                ).run_if(in_state(Sim::Running)),
            );
    }
}
