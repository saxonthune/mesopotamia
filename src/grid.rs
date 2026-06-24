use bevy::prelude::*;

use crate::field;
use crate::sim::Sim;

pub const GRID_WIDTH: usize = 256;
pub const GRID_HEIGHT: usize = 108;

pub const MAX_GRASS: f32 = 1.0;
pub const MAX_POOP: f32 = 1.0;
pub const MAX_WATER: f32 = 1.0;
pub const MAX_SHRUBS: f32 = 1.0;
pub const MAX_ROUGH: f32 = 1.0;

#[derive(Resource)]
pub struct Grid {
    width: usize,
    height: usize,
    grass: Vec<f32>,
    poop: Vec<f32>,
    water: Vec<f32>,
    /// Grass carrying capacity per cell in [0, 1], driven by proximity to water.
    /// 0 inside water (grass can't grow), 1 right beside it, tapering to 0 by
    /// `WATER_REACH` cells away. Computed once when the river is generated.
    water_prox: Vec<f32>,
    /// Static soil-fertility multiplier in [0, 1], a low-frequency noise patch
    /// field. Folded into `capacity` so the smooth water-distance gradient breaks
    /// into rich thickets and poor scrapes. Authored once by `seed_soil`.
    soil: Vec<f32>,
    /// Shrub (big-leaf) standing crop per cell, [0, MAX_SHRUBS]. A second
    /// forage type living on dry ground, off the water cycle entirely.
    shrubs: Vec<f32>,
    /// Shrub carrying capacity — high on dry steppe, zero in/near water and off
    /// the shrub clumps. Authored on the first shrub-growth tick.
    shrub_cap: Vec<f32>,
    /// Ford mask: true on cells authored as periodic shallow crossings along the
    /// main channel centerline. Read by Phase B to apply the crossing discount.
    ford: Vec<bool>,
    /// Lake mask: true on cells filled by the metaball lake pass (as distinct from
    /// river/tributary channels). The water-proximity field gives lake cells a
    /// shorter bank reach than rivers, so lakes carry a tighter ring of grass.
    lake: Vec<bool>,
    /// Soil-type gradient: 0 = dry steppe, 1 = moist riparian. Derived from the
    /// water-proximity field after the water layer runs. Drives two-tone dirt render
    /// and a gentle shrub-capacity nudge away from the riparian band.
    soil_type: Vec<f32>,
    /// Rough-terrain presence/intensity per cell, [0, MAX_ROUGH]. Noise-thresholded
    /// patches of broken ground that clump on the dry steppe far from water and are
    /// excluded from water cells. Authored by `seed_rough`; purely data + render for
    /// now, with no traversal hookup.
    rough: Vec<f32>,
    /// Continental-divide classification: true when the cell lies strictly east of
    /// its nearest main river (by column). The watershed midway between adjacent
    /// rivers is just where this flips. Authored by `seed_river_side`; shrub shading
    /// reads it to tint east-side shrubs a lighter olive.
    east_of_river: Vec<bool>,
    /// Normalized [0, 1] distance to the nearest main river: 0 on a river, rising
    /// to 1 at the farthest interfluve (the divide). Distinct from `water_prox`,
    /// which also counts lakes — this is rivers only. Authored by `seed_river_side`;
    /// flower colour interpolates white (near water) → cyan (on the divide) along it.
    river_dist: Vec<f32>,
    /// Flower presence: true on a random subset of shrub-bearing cells. A flowering
    /// shrub draws a white→cyan diamond once it is fully grown. Authored by the
    /// vegetation layer alongside shrub capacity.
    flower: Vec<bool>,
    /// Cosmetic soil-texture tint id: 0 = plain tan, 1 = a 1×2 speckle a hair
    /// toward red, 2 = a 1×1 speckle a touch redder still. Breaks the flat tan
    /// steppe into a subtle grain. Authored by `seed_soil_texture`; render reads
    /// it to pick the dry-soil base colour.
    soil_tint: Vec<u8>,
    /// Per-cell forage freshness, in [0, ∞). Rises where grass is actively
    /// regrowing (the green-up front) and decays everywhere else. The herd's
    /// grass drive blends this into the attractiveness signal so elk steer
    /// toward the advancing fresh front rather than mature standing biomass.
    /// Sized like `grass`; initialised to 0; stepped each tick by `growth`.
    pub freshness: Vec<f32>,
}

/// How fast grass regrows under the reaction-diffusion model: a cell gains
/// `(intrinsic + spread * neighbour_cover) * (capacity - grass)` per tick, so a
/// grazed hole heals from its green rim inward rather than refilling uniformly.
#[derive(Resource)]
pub struct GrowthRate {
    /// Growth fraction with no green neighbour — slow colonisation of bare ground.
    pub intrinsic: f32,
    /// Extra growth per unit of neighbouring cover — recolonisation from the edge.
    pub spread: f32,
}

impl Default for GrowthRate {
    fn default() -> Self {
        Self { intrinsic: 0.02, spread: 0.08 }
    }
}

/// Tunables for the travelling green-up crest. `strength` 0 disables the wave
/// entirely (floor == intrinsic everywhere, no senescence — today's behaviour).
#[derive(Resource, Clone, Copy)]
pub struct GreenWave {
    /// 0 = off; scales crest depth AND senescence together.
    pub strength: f32,
    /// Cycles/tick the crest marches east. At `speed * wavelength` cells/tick.
    pub speed: f32,
    /// Crest spacing in cells. ~⅓–½ of GRID_WIDTH so a couple of bands span the map.
    pub wavelength: f32,
}

impl Default for GreenWave {
    fn default() -> Self {
        Self {
            strength: 0.5,
            // Crest advances 85 * 0.010 = 0.85 cells/tick; crosses 256 cells in ~300 ticks.
            speed: 0.010,
            wavelength: 85.0,
        }
    }
}

/// Simulation tick counter, incremented once per FixedUpdate tick while Running.
/// The grass growth wave reads this as a phase clock.
#[derive(Resource, Default)]
pub struct SimTick(pub u32);

impl Grid {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            grass: vec![0.0; width * height],
            poop: vec![0.0; width * height],
            water: vec![0.0; width * height],
            // Defaults to full capacity so grass grows normally until the river
            // generator overwrites this with the real proximity field.
            water_prox: vec![1.0; width * height],
            // Full fertility until `seed_soil` writes the patch field.
            soil: vec![1.0; width * height],
            shrubs: vec![0.0; width * height],
            shrub_cap: vec![0.0; width * height],
            ford: vec![false; width * height],
            lake: vec![false; width * height],
            soil_type: vec![0.0; width * height],
            rough: vec![0.0; width * height],
            east_of_river: vec![false; width * height],
            river_dist: vec![0.0; width * height],
            flower: vec![false; width * height],
            soil_tint: vec![0u8; width * height],
            freshness: vec![0.0; width * height],
        }
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn len(&self) -> usize {
        self.grass.len()
    }

    pub fn col_row(&self, index: usize) -> (usize, usize) {
        (index % self.width, index / self.width)
    }

    pub fn step(&self, index: usize, dx: isize, dy: isize) -> Option<usize> {
        let (col, row) = self.col_row(index);
        let new_col = col as isize + dx;
        let new_row = row as isize + dy;
        if new_col < 0
            || new_col >= self.width as isize
            || new_row < 0
            || new_row >= self.height as isize {
                return None;
        }
        Some(new_row as usize * self.width + new_col as usize)
    }

    pub fn grass(&self, index: usize) -> f32 {
        self.grass[index]
    }

    pub fn set_grass(&mut self, index: usize, value: f32) {
        let cap = self.capacity(index);
        self.grass[index] = value.clamp(0.0, cap);
    }

    /// Max grass this cell can hold: water proximity gated by soil fertility, so
    /// a cell needs both nearby water and good ground to carry much grass.
    pub fn capacity(&self, index: usize) -> f32 {
        self.water_prox[index] * self.soil[index] * MAX_GRASS
    }

    pub fn set_water_prox(&mut self, index: usize, value: f32) {
        self.water_prox[index] = value.clamp(0.0, 1.0);
    }

    pub fn water_prox(&self, index: usize) -> f32 {
        self.water_prox[index]
    }

    /// Set the shrub carrying capacity for a cell. Authored by the
    /// world-gen vegetation layer; the runtime shrub regrowth reads it.
    pub fn set_shrub_cap(&mut self, index: usize, value: f32) {
        self.shrub_cap[index] = value.clamp(0.0, MAX_SHRUBS);
    }

    #[cfg(test)]
    pub fn soil(&self, index: usize) -> f32 {
        self.soil[index]
    }

    pub fn set_soil(&mut self, index: usize, value: f32) {
        self.soil[index] = value.clamp(0.0, 1.0);
    }

    pub fn grow_grass(&mut self, index: usize, amount: f32)
    {
        self.set_grass(index, self.grass[index] + amount);
    }

    pub fn poop(&self, index: usize) -> f32 {
        self.poop[index]
    }

    pub fn add_poop(&mut self, index: usize, amount: f32) {
        self.poop[index] = (self.poop[index] + amount).clamp(0.0, MAX_POOP);
    }

    pub fn water(&self, index: usize) -> f32 {
        self.water[index]
    }

    pub fn set_water(&mut self, index: usize, value: f32) {
        self.water[index] = value.clamp(0.0, MAX_WATER);
    }

    // Phase B (water-as-barrier-fords) reads this to apply the crossing discount.
    #[allow(dead_code)]
    pub fn is_ford(&self, index: usize) -> bool {
        self.ford[index]
    }

    pub fn set_ford(&mut self, index: usize, value: bool) {
        self.ford[index] = value;
    }

    /// Whether this cell was filled by the lake pass. Read by the water-proximity
    /// field to give lakes a shorter bank reach than rivers.
    pub fn is_lake(&self, index: usize) -> bool {
        self.lake[index]
    }

    /// Tag a cell as lake water. Authored by the lake pass alongside `set_water`.
    pub fn set_lake(&mut self, index: usize, value: bool) {
        self.lake[index] = value;
    }

    pub fn soil_type(&self, index: usize) -> f32 {
        self.soil_type[index]
    }

    pub fn set_soil_type(&mut self, index: usize, value: f32) {
        self.soil_type[index] = value.clamp(0.0, 1.0);
    }

    pub fn rough(&self, index: usize) -> f32 {
        self.rough[index]
    }

    /// Set rough-terrain intensity for a cell. Authored by the world-gen rough
    /// layer; the render reads it to override tan soil with grey-brown.
    pub fn set_rough(&mut self, index: usize, value: f32) {
        self.rough[index] = value.clamp(0.0, MAX_ROUGH);
    }

    /// Whether this cell lies strictly east of its nearest main river. Read by
    /// shrub shading to pick the olive (east) vs green (west) tint.
    pub fn east_of_river(&self, index: usize) -> bool {
        self.east_of_river[index]
    }

    /// Set the continental-divide classification for a cell. Authored by
    /// `seed_river_side` from the nearest-river Voronoi assignment.
    pub fn set_east_of_river(&mut self, index: usize, value: bool) {
        self.east_of_river[index] = value;
    }

    /// Normalized [0, 1] distance to the nearest main river. Read by flower
    /// shading to interpolate white (near water) → cyan (on the divide).
    pub fn river_dist(&self, index: usize) -> f32 {
        self.river_dist[index]
    }

    /// Set the normalized river-distance for a cell. Authored by `seed_river_side`.
    pub fn set_river_dist(&mut self, index: usize, value: f32) {
        self.river_dist[index] = value.clamp(0.0, 1.0);
    }

    /// Whether this cell bears a flower. Read by flower rendering, gated on the
    /// shrub being fully grown.
    pub fn flower(&self, index: usize) -> bool {
        self.flower[index]
    }

    /// Set flower presence for a cell. Authored by the vegetation layer.
    pub fn set_flower(&mut self, index: usize, value: bool) {
        self.flower[index] = value;
    }

    /// Shrub carrying capacity for a cell — the ceiling the standing crop grows
    /// toward. Read by flower rendering to decide "fully grown".
    pub fn shrub_cap(&self, index: usize) -> f32 {
        self.shrub_cap[index]
    }

    /// Cosmetic soil-texture tint id (0/1/2). Read by render to pick the dry-soil
    /// base colour.
    pub fn soil_tint(&self, index: usize) -> u8 {
        self.soil_tint[index]
    }

    /// Set the soil-texture tint id. Authored by `seed_soil_texture`.
    pub fn set_soil_tint(&mut self, index: usize, value: u8) {
        self.soil_tint[index] = value;
    }

    pub fn shrubs(&self, index: usize) -> f32 {
        self.shrubs[index]
    }

    /// Seed the standing shrub crop for a cell, clamped to its carrying capacity.
    /// World generation calls this so a freshly generated world starts with mature
    /// shrubs already in place rather than growing them up from bare ground; the
    /// runtime regrowth then merely maintains them. Set `shrub_cap` first.
    pub fn set_shrubs(&mut self, index: usize, value: f32) {
        self.shrubs[index] = value.clamp(0.0, self.shrub_cap[index]);
    }

    /// Strip shrubs from a cell (positive `amount` removes it). Floors at zero.
    pub fn eat_shrubs(&mut self, index: usize, amount: f32) {
        self.shrubs[index] = (self.shrubs[index] - amount).max(0.0);
    }

    /// Per-cell forage freshness — how much recent regrowth this cell has seen.
    /// Peaks at the active green-up front (the wave crest) and decays away from it.
    pub fn freshness(&self, index: usize) -> f32 {
        self.freshness[index]
    }

    /// Total forage an elk perceives at a cell — grass plus shrubs. The herd's
    /// grass-seeking drive steers up this combined field, so shrub clumps pull
    /// foragers the same way rich grass does.
    pub fn forage(&self, index: usize) -> f32 {
        self.grass[index] + self.shrubs[index]
    }

    /// Total standing grass biomass across the whole grid — Σ grass. A macro
    /// reduction the UI samples into `History` for the biomass graph; the
    /// simulation itself never reads it.
    pub fn total_grass(&self) -> f32 {
        self.grass.iter().sum()
    }

    /// Total standing shrub biomass across the whole grid — Σ shrubs.
    pub fn total_shrubs(&self) -> f32 {
        self.shrubs.iter().sum()
    }

    /// Clear all per-cell state back to a freshly-constructed world, preserving
    /// dimensions. World generation calls this before re-stamping, so regeneration
    /// starts from a clean substrate instead of layering onto the previous world.
    pub fn reset(&mut self) {
        *self = Self::new(self.width, self.height);
    }
}

pub struct GridPlugin;

// Shrubs (dry-ground) regrow slowly in place toward the capacity the
// world-gen vegetation layer authored — a long lifecycle.
const SHRUB_REGROW: f32 = 0.0025;

impl Plugin for GridPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Grid::new(GRID_WIDTH, GRID_HEIGHT))
            .init_resource::<GrowthRate>()
            .init_resource::<GreenWave>()
            .init_resource::<SimTick>()
            .add_systems(
                FixedUpdate,
                (tick_counter, growth, grow_shrubs)
                    .chain()
                    .run_if(in_state(Sim::Running)),
            );
    }
}

fn tick_counter(mut tick: ResMut<SimTick>) {
    tick.0 += 1;
}

// Crest/trough asymmetry factors. Both set to 1.0 so the wavelength-mean floor
// equals `intrinsic` exactly: E[crest_gain·w − trough_cut·(1−w)] over w∈[0,1] is
// 0.5*(crest_gain − trough_cut) = 0 when they are equal.
const CREST_GAIN: f32 = 1.0;
const TROUGH_CUT: f32 = 1.0;
/// Fraction of freshness that decays each tick — slow so a fresh band persists
/// for several ticks as the crest moves through (~20 ticks half-life at 0.05).
const FRESH_DECAY: f32 = 0.05;
/// Freshness gain per unit of grass growth — amplifies the small per-tick regrowth
/// signal into a perceivable freshness band at the crest.
const FRESH_GAIN: f32 = 8.0;
// Maximum fractional senescence per tick, applied at the trough.  At strength=0.5
// a trough cell loses ≤2.5% of its standing crop per tick — gentle enough that a
// grazing herd keeps up, strong enough to clear stale ungrazed trough grass.
const SENESCE_MAX: f32 = 0.05;

/// Reaction-diffusion regrowth: grass spreads from green neighbours toward each
/// cell's capacity, modulated by the green-wave crest so fresh grass builds at the
/// advancing crest and ungrazed stale grass senesces in the trough behind it.
/// At `GreenWave.strength == 0` every cell uses `rate.intrinsic` and zero
/// senescence — exactly today's behaviour.
fn growth(mut grid: ResMut<Grid>, rate: Res<GrowthRate>, wave: Res<GreenWave>, tick: Res<SimTick>) {
    let n = grid.len();
    let width = grid.width();
    let height = grid.height();
    let caps: Vec<f32> = (0..n).map(|i| grid.capacity(i)).collect();
    let t = tick.0 as f32;

    let mut floor = vec![0.0f32; n];
    let mut senesce = vec![0.0f32; n];
    for i in 0..n {
        let col = i % width;
        let w = field::green_wave(col, wave.wavelength, t, wave.speed);
        floor[i] = (rate.intrinsic
            * (1.0 + wave.strength * (CREST_GAIN * w - TROUGH_CUT * (1.0 - w))))
            .max(0.0);
        senesce[i] = wave.strength * SENESCE_MAX * (1.0 - w);
    }

    let prev_grass = grid.grass.clone();
    let next_grass = field::wave_grow(&prev_grass, &caps, width, height, &floor, rate.spread, &senesce);
    let grew: Vec<f32> = (0..n)
        .map(|i| (next_grass[i] - prev_grass[i]).max(0.0))
        .collect();
    grid.freshness = field::step_freshness(&grid.freshness, &grew, FRESH_DECAY, FRESH_GAIN);
    grid.grass = next_grass;
}

/// Shrubs grow slowly toward their dry-ground capacity with a pure logistic step
/// (`spread = 0` → no colonisation; shrubs regenerate in place). Capacity is
/// authored by the world-gen vegetation layer before the sim runs.
fn grow_shrubs(mut grid: ResMut<Grid>) {
    grid.shrubs = field::spread_grow(
        &grid.shrubs,
        &grid.shrub_cap,
        grid.width(),
        grid.height(),
        SHRUB_REGROW,
        0.0,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    // Total biomass is the plain sum over every cell — the macro reduction the
    // biomass graph plots.
    #[test]
    fn total_grass_sums_every_cell() {
        let mut grid = Grid::new(2, 2);
        grid.set_grass(0, 0.5);
        grid.set_grass(3, 0.25);
        assert!((grid.total_grass() - 0.75).abs() < 1e-6);
    }

    #[test]
    fn reset_clears_all_per_cell_state() {
        let mut grid = Grid::new(4, 4);
        // Mutate several fields.
        grid.set_grass(0, 0.8);
        grid.add_poop(1, 0.5);
        grid.set_water(2, 1.0);
        grid.set_water_prox(3, 0.3);
        grid.set_soil(4, 0.2);

        grid.reset();

        // Grass is cleared.
        assert!((grid.total_grass()).abs() < 1e-6);
        // Poop is cleared.
        assert!((grid.poop(1)).abs() < 1e-6);
        // Water is cleared.
        assert!((grid.water(2)).abs() < 1e-6);
        // water_prox resets to 1.0 (Grid::new default).
        assert!((grid.water_prox(3) - 1.0).abs() < 1e-6);
        // soil resets to 1.0.
        assert!((grid.soil(4) - 1.0).abs() < 1e-6);
        // Dimensions are preserved.
        assert_eq!(grid.width(), 4);
        assert_eq!(grid.height(), 4);
    }
}

