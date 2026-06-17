//! World generation as a standalone concern: a *producer* that pours a finished
//! world into the `Grid` substrate the ECS then inherits. The orchestrator runs
//! the layers in one explicit, readable sequence — water, then soil, then
//! vegetation — each step calling into its own algorithm module. `Grid` owns
//! storage and runtime dynamics (growth, grazing, fertilising); it does not own
//! generation. Adding a layer means adding a module and one line to `generate_world`.

mod soil;
mod vegetation;

use bevy::prelude::*;

use crate::grid::Grid;
use crate::river::{generate_water, RiverSpec};

pub struct WorldgenPlugin;

impl Plugin for WorldgenPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, generate_world);
    }
}

/// Declarative authoring surface for the whole world: it composes each layer's
/// spec. Today only the watershed is parameterised; later layers add their own
/// fields here so the world stays authored from one place.
pub struct WorldSpec {
    pub river: RiverSpec,
}

impl Default for WorldSpec {
    fn default() -> Self {
        Self { river: RiverSpec::default() }
    }
}

/// The world-generation orchestrator. The call order *is* the contract: soil and
/// vegetation both read the water field, so the water layer runs first, then they
/// follow — explicitly, in sequence, rather than via scheduling side effects.
fn generate_world(mut grid: ResMut<Grid>) {
    let spec = WorldSpec::default();

    // 1. Water — rivers, tributaries, lakes, ford bands, and the water-proximity
    //    carrying-capacity field every later layer leans on.
    generate_water(&mut grid, &spec.river);

    // 2. Soil — static fertility patches multiplied by a coarse regional octave.
    soil::seed_soil(&mut grid);

    // 3. Vegetation — browse (shrub) capacity on the dry ground away from water.
    vegetation::seed_browse_cap(&mut grid);
}
