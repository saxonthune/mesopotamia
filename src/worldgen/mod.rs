//! World generation as a standalone concern: a *producer* that pours a finished
//! world into the `Grid` substrate the ECS then inherits. The orchestrator runs
//! the layers in one explicit, readable sequence — water, then soil, then
//! vegetation — each step calling into its own algorithm module. `Grid` owns
//! storage and runtime dynamics (growth, grazing, fertilising); it does not own
//! generation. Adding a layer means adding a module and one line to `generate_world`.

mod river_side;
mod rough;
mod soil;
mod soil_texture;
mod soil_type;
mod vegetation;

use bevy::prelude::*;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use crate::grid::Grid;
use crate::river::{generate_water, random_confluences, RiverSpec};
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
    pub rough_seed: u64,
    pub soil_texture_seed: u64,
    pub shrub_seed: u64,
}

impl WorldSpec {
    /// Derive a full set of decorrelated per-layer seeds from one master seed.
    /// Each layer draws its own seed from a master-seeded RNG, so the layers'
    /// heterogeneities stay independent while the whole world reduces to one value.
    pub fn from_seed(seed: u64) -> Self {
        let mut rng = StdRng::seed_from_u64(seed);
        // Four mains. Confluence is randomized per world (rather than the spec
        // default's fixed pair) so the two rivers that collide differ each seed
        // instead of always being mains 0 and 1.
        let river_seed = rng.random();
        let count = 4;
        let confluence_pairs = random_confluences(count, &mut rng);
        Self {
            river: RiverSpec { seed: river_seed, count, confluence_pairs, ..default() },
            soil_seed: rng.random(),
            macro_seed: rng.random(),
            rough_seed: rng.random(),
            soil_texture_seed: rng.random(),
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
    //    carrying-capacity field every later layer leans on. Keep the main-river
    //    centerlines (`mains`) for the continental-divide classification below.
    let (mains, _tribs) = generate_water(&mut grid, &spec.river);

    // 2. River side — the invisible continental-divide classification: a Voronoi
    //    assignment of every cell to its nearest main river, tagging cells strictly
    //    east of that river. Needs only the centerlines and the grid; shrub shading
    //    reads it to tint east-side shrubs olive.
    river_side::seed_river_side(&mut grid, &mains);

    // 3. Soil — static fertility patches multiplied by a coarse regional octave.
    soil::seed_soil(&mut grid, spec.soil_seed, spec.macro_seed);

    // 4. Soil type — riparian/steppe gradient derived from water proximity.
    soil_type::seed_soil_type(&mut grid);

    // 5. Rough terrain — broken-ground glyphs on the dry steppe away from water.
    //    Runs after soil_type (reads the already-computed water field) and before
    //    vegetation, which reads the rough field it lays down.
    rough::seed_rough(&mut grid, spec.rough_seed);

    // 6. Soil texture — a sparse reddish speckle over the open tan steppe so the
    //    dry ground reads with grain. Reads water + rough so the speckle avoids
    //    channels and broken ground; purely cosmetic.
    soil_texture::seed_soil_texture(&mut grid, spec.soil_texture_seed);

    // 7. Vegetation — shrub capacity on the dry ground away from water, grown
    //    *around* the rough glyphs with a blank border so the two stay distinct.
    vegetation::seed_shrub_cap(&mut grid, spec.shrub_seed);

    next.set(Sim::Running);
}
