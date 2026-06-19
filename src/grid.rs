use bevy::prelude::*;

use crate::field;
use crate::sim::Sim;

pub const GRID_WIDTH: usize = 256;
pub const GRID_HEIGHT: usize = 108;

pub const MAX_GRASS: f32 = 1.0;
pub const MAX_POOP: f32 = 1.0;
pub const MAX_WATER: f32 = 1.0;
pub const MAX_SHRUBS: f32 = 1.0;

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
    /// Soil-type gradient: 0 = dry steppe, 1 = moist riparian. Derived from the
    /// water-proximity field after the water layer runs. Drives two-tone dirt render
    /// and a gentle shrub-capacity nudge away from the riparian band.
    soil_type: Vec<f32>,
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
            soil_type: vec![0.0; width * height],
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

    pub fn soil_type(&self, index: usize) -> f32 {
        self.soil_type[index]
    }

    pub fn set_soil_type(&mut self, index: usize, value: f32) {
        self.soil_type[index] = value.clamp(0.0, 1.0);
    }

    pub fn shrubs(&self, index: usize) -> f32 {
        self.shrubs[index]
    }

    /// Strip shrubs from a cell (positive `amount` removes it). Floors at zero.
    pub fn eat_shrubs(&mut self, index: usize, amount: f32) {
        self.shrubs[index] = (self.shrubs[index] - amount).max(0.0);
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
            .add_systems(FixedUpdate, (growth, grow_shrubs).run_if(in_state(Sim::Running)));
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

