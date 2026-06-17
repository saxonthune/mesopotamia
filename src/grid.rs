use bevy::prelude::*;
use rand::SeedableRng;
use rand::rngs::StdRng;

use crate::field;

pub const GRID_WIDTH: usize = 256;
pub const GRID_HEIGHT: usize = 108;

pub const MAX_GRASS: f32 = 1.0;
pub const MAX_POOP: f32 = 1.0;
pub const MAX_WATER: f32 = 1.0;
pub const MAX_BROWSE: f32 = 1.0;

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
    /// Browse (big-leaf shrub) standing crop per cell, [0, MAX_BROWSE]. A second
    /// forage type living on dry ground, off the water cycle entirely.
    browse: Vec<f32>,
    /// Browse carrying capacity — high on dry steppe, zero in/near water and off
    /// the shrub clumps. Authored on the first browse-growth tick.
    browse_cap: Vec<f32>,
    /// Ford mask: true on cells authored as periodic shallow crossings along the
    /// main channel centerline. Read by Phase B to apply the crossing discount.
    ford: Vec<bool>,
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
            browse: vec![0.0; width * height],
            browse_cap: vec![0.0; width * height],
            ford: vec![false; width * height],
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

    pub fn browse(&self, index: usize) -> f32 {
        self.browse[index]
    }

    /// Strip browse from a cell (positive `amount` removes it). Floors at zero.
    pub fn eat_browse(&mut self, index: usize, amount: f32) {
        self.browse[index] = (self.browse[index] - amount).max(0.0);
    }

    /// Total forage an elk perceives at a cell — grass plus browse. The herd's
    /// grass-seeking drive steers up this combined field, so shrub clumps pull
    /// foragers the same way rich grass does.
    pub fn forage(&self, index: usize) -> f32 {
        self.grass[index] + self.browse[index]
    }
}

pub struct GridPlugin;

// Soil patch field. A distinct seed from the river so the two heterogeneities
// (where water is vs. where the ground is rich) are uncorrelated.
const SOIL_SEED: u64 = 0x501;
const SOIL_PASSES: usize = 6; // smoothing passes — higher = broader patches
const SOIL_FLOOR: f32 = 0.35; // the poorest ground still carries this fraction

// Macro (coarse) regional fertility octave — broad lush vs. sparse zones.
// A distinct seed so coarse regions are uncorrelated with fine patches.
const MACRO_SEED: u64 = 0xA7C3;
const MACRO_PASSES: usize = 14; // many passes → features span large fractions of the map
const MACRO_FLOOR: f32 = 0.4; // the sparsest region still carries this fraction

// Browse (dry-ground shrubs). A separate seed and patch from soil/river so the
// shrub clumps fall independently of where water and good grazing land are.
const BROWSE_SEED: u64 = 0xB405E;
const BROWSE_PASSES: usize = 5;
const BROWSE_THRESHOLD: f32 = 0.55; // only the densest noise becomes a shrub clump
const BROWSE_REGROW: f32 = 0.0025; // slow logistic regrowth — a long lifecycle

impl Plugin for GridPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Grid::new(GRID_WIDTH, GRID_HEIGHT))
            .init_resource::<GrowthRate>()
            .init_resource::<Fertility>()
            .add_systems(Startup, seed_soil)
            .add_systems(FixedUpdate, (growth, grow_browse, fertilize));
    }
}

/// Author the static patchiness field (#1): fine low-frequency value noise (soil
/// patches) multiplied by a coarse macro-octave (regional lush/sparse zones).
/// Both factors are in (0, 1] so `soil` stays in [0, 1]; `capacity` inherits the
/// regional variation automatically.
fn seed_soil(mut grid: ResMut<Grid>) {
    let mut rng = StdRng::seed_from_u64(SOIL_SEED);
    let noise = field::value_noise(grid.width(), grid.height(), SOIL_PASSES, &mut rng);
    let patch = field::normalize(&noise);

    let mut macro_rng = StdRng::seed_from_u64(MACRO_SEED);
    let macro_noise = field::value_noise(grid.width(), grid.height(), MACRO_PASSES, &mut macro_rng);
    let macro_patch = field::normalize(&macro_noise);

    for (index, (&p, &m)) in patch.iter().zip(macro_patch.iter()).enumerate() {
        let fine   = SOIL_FLOOR + (1.0 - SOIL_FLOOR) * p;
        let region = MACRO_FLOOR + (1.0 - MACRO_FLOOR) * m;
        grid.set_soil(index, fine * region);
    }
}

/// Reaction-diffusion regrowth (#3): grass spreads from green neighbours toward
/// each cell's capacity. Domain wiring over `field::spread_grow`.
fn growth(mut grid: ResMut<Grid>, rate: Res<GrowthRate>) {
    let caps: Vec<f32> = (0..grid.len()).map(|i| grid.capacity(i)).collect();
    grid.grass = field::spread_grow(
        &grid.grass,
        &caps,
        grid.width(),
        grid.height(),
        rate.intrinsic,
        rate.spread,
    );
}

/// Browse grows slowly toward its dry-ground capacity with a pure logistic step
/// (`spread = 0` → no colonisation; shrubs regenerate in place). Capacity is
/// authored on the first tick, once the river's water field exists.
fn grow_browse(mut grid: ResMut<Grid>, mut seeded: Local<bool>) {
    if !*seeded {
        seed_browse_cap(&mut grid);
        *seeded = true;
    }
    grid.browse = field::spread_grow(
        &grid.browse,
        &grid.browse_cap,
        grid.width(),
        grid.height(),
        BROWSE_REGROW,
        0.0,
    );
}

/// Author browse capacity: dense noise clumps on dry ground (far from water),
/// excluded from water cells. Reads `water`/`water_prox` straight off the grid,
/// which the river generator has filled by the time the first FixedUpdate runs.
fn seed_browse_cap(grid: &mut Grid) {
    let mut rng = StdRng::seed_from_u64(BROWSE_SEED);
    let patch = field::normalize(&field::value_noise(
        grid.width(),
        grid.height(),
        BROWSE_PASSES,
        &mut rng,
    ));
    for (i, &p) in patch.iter().enumerate() {
        // Dryness: far from water → 1, beside it → 0. Browse wants the steppe.
        let dry = 1.0 - grid.water_prox[i];
        let cap = if grid.water[i] > 0.0 || p < BROWSE_THRESHOLD {
            0.0
        } else {
            dry * MAX_BROWSE
        };
        grid.browse_cap[i] = cap.clamp(0.0, MAX_BROWSE);
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

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    /// `seed_soil` must produce a field with genuine regional spread — both the
    /// macro octave and the fine patches are blended in, so the resulting soil has
    /// a wide cell-level range and is bounded within [0, 1].
    /// Uses fixed seeds → deterministic.
    #[test]
    fn soil_has_regional_spread() {
        let mut grid = Grid::new(GRID_WIDTH, GRID_HEIGHT);

        // Replicate the seed_soil logic inline so the test owns the grid directly.
        let mut rng = StdRng::seed_from_u64(SOIL_SEED);
        let noise = crate::field::value_noise(grid.width(), grid.height(), SOIL_PASSES, &mut rng);
        let patch = crate::field::normalize(&noise);

        let mut macro_rng = StdRng::seed_from_u64(MACRO_SEED);
        let macro_noise = crate::field::value_noise(grid.width(), grid.height(), MACRO_PASSES, &mut macro_rng);
        let macro_patch = crate::field::normalize(&macro_noise);

        for (index, (&p, &m)) in patch.iter().zip(macro_patch.iter()).enumerate() {
            let fine   = SOIL_FLOOR + (1.0 - SOIL_FLOOR) * p;
            let region = MACRO_FLOOR + (1.0 - MACRO_FLOOR) * m;
            grid.set_soil(index, fine * region);
        }

        let n = grid.len();
        let min: f32 = (0..n).map(|i| grid.soil(i)).fold(f32::MAX, f32::min);
        let max: f32 = (0..n).map(|i| grid.soil(i)).fold(f32::MIN, f32::max);

        println!("soil range: min={min:.4} max={max:.4} spread={:.4}", max - min);

        // Values must stay in [0, 1] (clamped by set_soil).
        assert!(min >= 0.0 && max <= 1.0, "soil out of [0,1]: min={min} max={max}");

        // The macro octave must produce substantial regional contrast: the cell-level
        // spread must exceed 0.3 (with fine floor 0.35 and macro floor 0.4, the
        // theoretical range is [0.14, 1.0]; in practice the normalised noise uses
        // the full [0,1] range, so we expect a spread of at least 0.4).
        assert!(
            max - min > 0.3,
            "regional spread too narrow: soil range = {:.4} (min={min:.4} max={max:.4})",
            max - min
        );
    }
}