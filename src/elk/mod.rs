mod components;
mod color;
mod spawn;
mod movement;
mod metabolism;

pub use components::{Cohort, Elk, ElkParams, Herds, Packs, Spawner};
#[allow(unused_imports)]
pub use movement::{combine_drives, cross_desire, migration_residual, step_water_penalty, Drives};
pub(crate) use color::elk_color;

use bevy::prelude::*;

pub struct ElkSimPlugin;

pub const PACK_COUNT: usize = 8;

impl Plugin for ElkSimPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Packs::new())
            .init_resource::<Spawner>()
            .init_resource::<ElkParams>()
            .init_resource::<Herds>()
            .add_systems(Update, spawn::tally_herds)
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
                ),
            );
    }
}
