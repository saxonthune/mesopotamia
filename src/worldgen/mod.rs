//! World generation as a standalone concern: a *producer* that pours a finished
//! world into the `Grid` substrate the ECS then inherits. The orchestrator runs
//! the layers in one explicit, readable sequence — water, then soil, then
//! vegetation — each step calling into its own algorithm module. `Grid` owns
//! storage and runtime dynamics (growth, grazing, fertilising); it does not own
//! generation. Adding a layer means adding a module and one line to `generate_world`.

mod soil;
mod soil_type;
mod vegetation;

use bevy::prelude::*;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use crate::grid::Grid;
use crate::river::{generate_water, RiverSpec};
use crate::sim::Sim;

pub struct WorldgenPlugin;

impl Plugin for WorldgenPlugin {
    fn build(&self, app: &mut App) {
        // Default to a fresh world each run by drawing one master seed from OS
        // entropy. A binary or a future regenerate control can overwrite the
        // `WorldSeed` resource before `generate_world` runs to pin or replay a world.
        app.insert_resource(WorldSeed(rand::random()))
            .add_systems(OnEnter(Sim::Generating), generate_world);
    }
}

/// The single master seed every world-generation layer derives from. The whole
/// world is a pure function of this one value: same seed → same world, fresh seed
/// → fresh world. Held as a resource so it can be set per run, logged for replay,
/// or overwritten to regenerate.
#[derive(Resource, Clone, Copy, Debug)]
pub struct WorldSeed(pub u64);

/// Declarative authoring surface for the whole world: it composes each layer's
/// seed and spec. The layer seeds are decorrelated draws from one master seed, so
/// the world stays authored — and reproducible — from a single value.
pub struct WorldSpec {
    pub river: RiverSpec,
    pub soil_seed: u64,
    pub macro_seed: u64,
    pub shrub_seed: u64,
}

impl WorldSpec {
    /// Derive a full set of decorrelated per-layer seeds from one master seed.
    /// Each layer draws its own seed from a master-seeded RNG, so the layers'
    /// heterogeneities stay independent while the whole world reduces to one value.
    pub fn from_seed(seed: u64) -> Self {
        let mut rng = StdRng::seed_from_u64(seed);
        Self {
            river: RiverSpec { seed: rng.random(), ..default() },
            soil_seed: rng.random(),
            macro_seed: rng.random(),
            shrub_seed: rng.random(),
        }
    }
}

/// The world-generation orchestrator. The call order *is* the contract: soil and
/// vegetation both read the water field, so the water layer runs first, then they
/// follow — explicitly, in sequence, rather than via scheduling side effects.
fn generate_world(mut grid: ResMut<Grid>, seed: Res<WorldSeed>, mut next: ResMut<NextState<Sim>>) {
    grid.reset();
    // Log the seed so a world worth keeping can be replayed by pinning this value.
    info!("worldgen master seed = {:#018x}", seed.0);
    let spec = WorldSpec::from_seed(seed.0);

    // 1. Water — rivers, tributaries, lakes, ford bands, and the water-proximity
    //    carrying-capacity field every later layer leans on.
    generate_water(&mut grid, &spec.river);

    // 2. Soil — static fertility patches multiplied by a coarse regional octave.
    soil::seed_soil(&mut grid, spec.soil_seed, spec.macro_seed);

    // 3. Soil type — riparian/steppe gradient derived from water proximity.
    soil_type::seed_soil_type(&mut grid);

    // 4. Vegetation — shrub capacity on the dry ground away from water.
    vegetation::seed_shrub_cap(&mut grid, spec.shrub_seed);

    next.set(Sim::Running);
}
