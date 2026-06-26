//! World generation: pours a finished world into `Grid`. Layer order is the contract —
//! water first, then soil, then vegetation — each layer may read what the previous wrote.

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
        // Default: random OS seed each run; overwrite `WorldSeed` before `generate_world` to pin or replay.
        app.insert_resource(WorldSeed(rand::random()))
            .add_systems(OnEnter(Sim::Generating), generate_world);
    }
}

/// Master seed: same value → same world. Set before `generate_world` to pin or replay.
#[derive(Resource, Clone, Copy, Debug)]
pub struct WorldSeed(pub u64);

pub struct WorldSpec {
    pub river: RiverSpec,
    pub soil_seed: u64,
    pub macro_seed: u64,
    pub rough_seed: u64,
    pub soil_texture_seed: u64,
    pub shrub_seed: u64,
}

impl WorldSpec {
    pub fn from_seed(seed: u64) -> Self {
        let mut rng = StdRng::seed_from_u64(seed);
        // Confluences randomized per world so the merging pair differs each seed.
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

/// Layer call order is the contract: each step reads fields the prior step wrote.
fn generate_world(mut grid: ResMut<Grid>, seed: Res<WorldSeed>, mut next: ResMut<NextState<Sim>>) {
    grid.reset();
    info!("worldgen master seed = {:#018x}", seed.0);
    let spec = WorldSpec::from_seed(seed.0);

    // 1. Water — rivers, proximity field, and main centerlines for step 2.
    let (mains, _tribs) = generate_water(&mut grid, &spec.river);

    // 2. River side — Voronoi east/west divide; shrub shading reads east_of_river.
    river_side::seed_river_side(&mut grid, &mains);

    // 3. Soil — fine patches × coarse regional octave.
    soil::seed_soil(&mut grid, spec.soil_seed, spec.macro_seed);

    // 4. Soil type — riparian/steppe gradient from water proximity.
    soil_type::seed_soil_type(&mut grid);

    // 5. Rough — broken-ground strokes; runs after soil_type, before vegetation (vegetation reads rough).
    rough::seed_rough(&mut grid, spec.rough_seed);

    // 6. Soil texture — cosmetic speckle in the rough apron; reads water + rough.
    soil_texture::seed_soil_texture(&mut grid, spec.soil_texture_seed);

    // 7. Vegetation — shrub capacity around rough glyphs.
    vegetation::seed_shrub_cap(&mut grid, spec.shrub_seed);

    next.set(Sim::Running);
}
