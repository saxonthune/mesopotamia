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
    water_prox: Vec<f32>,
    soil: Vec<f32>,
    shrubs: Vec<f32>,
    shrub_cap: Vec<f32>,
    // Phase B reads this to apply the crossing discount.
    ford: Vec<bool>,
    // Lakes get a shorter bank reach than rivers in the water-proximity field.
    lake: Vec<bool>,
    // 0 = dry steppe, 1 = moist riparian; drives two-tone dirt render and shrub-cap nudge.
    soil_type: Vec<f32>,
    rough: Vec<f32>,
    // True east of nearest main river; shrub shading tints east-side shrubs olive.
    east_of_river: Vec<bool>,
    // Rivers only (unlike water_prox which counts lakes); flower colour interpolates along it.
    river_dist: Vec<f32>,
    flower: Vec<bool>,
    // 0 = plain tan, 1 = 1×2 speckle, 2 = 1×1 speckle; cosmetic grain on dry soil.
    soil_tint: Vec<u8>,
    // Rises where grass regrows, decays elsewhere; steers elk toward the fresh front not standing biomass.
    pub freshness: Vec<f32>,
}

// Growth rate: `(intrinsic + spread * neighbour_cover) * (capacity - grass)` per tick.
#[derive(Resource)]
pub struct GrowthRate {
    pub intrinsic: f32,
    pub spread: f32,
    pub shrub: f32,
}

impl Default for GrowthRate {
    fn default() -> Self {
        Self { intrinsic: 0.02, spread: 0.08, shrub: 0.0025 }
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
            water_prox: vec![1.0; width * height], // overwritten by river generator
            soil: vec![1.0; width * height],        // overwritten by seed_soil
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

    pub fn capacity(&self, index: usize) -> f32 {
        self.water_prox[index] * self.soil[index] * MAX_GRASS
    }

    pub fn set_water_prox(&mut self, index: usize, value: f32) {
        self.water_prox[index] = value.clamp(0.0, 1.0);
    }

    pub fn water_prox(&self, index: usize) -> f32 {
        self.water_prox[index]
    }

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

    pub fn is_lake(&self, index: usize) -> bool {
        self.lake[index]
    }

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

    pub fn set_rough(&mut self, index: usize, value: f32) {
        self.rough[index] = value.clamp(0.0, MAX_ROUGH);
    }

    pub fn east_of_river(&self, index: usize) -> bool {
        self.east_of_river[index]
    }

    pub fn set_east_of_river(&mut self, index: usize, value: bool) {
        self.east_of_river[index] = value;
    }

    pub fn river_dist(&self, index: usize) -> f32 {
        self.river_dist[index]
    }

    pub fn set_river_dist(&mut self, index: usize, value: f32) {
        self.river_dist[index] = value.clamp(0.0, 1.0);
    }

    pub fn flower(&self, index: usize) -> bool {
        self.flower[index]
    }

    pub fn set_flower(&mut self, index: usize, value: bool) {
        self.flower[index] = value;
    }

    pub fn shrub_cap(&self, index: usize) -> f32 {
        self.shrub_cap[index]
    }

    pub fn soil_tint(&self, index: usize) -> u8 {
        self.soil_tint[index]
    }

    pub fn set_soil_tint(&mut self, index: usize, value: u8) {
        self.soil_tint[index] = value;
    }

    pub fn shrubs(&self, index: usize) -> f32 {
        self.shrubs[index]
    }

    // Requires shrub_cap to be set first (clamps to it).
    pub fn set_shrubs(&mut self, index: usize, value: f32) {
        self.shrubs[index] = value.clamp(0.0, self.shrub_cap[index]);
    }

    pub fn eat_shrubs(&mut self, index: usize, amount: f32) {
        self.shrubs[index] = (self.shrubs[index] - amount).max(0.0);
    }

    pub fn freshness(&self, index: usize) -> f32 {
        self.freshness[index]
    }

    // Exposed so headless probes can seed a fresh front without running the wave.
    pub fn set_freshness(&mut self, index: usize, value: f32) {
        self.freshness[index] = value.max(0.0);
    }

    // Food boundary: all behaviour reads food via forage/food_capacity/food_frac, never grass or shrubs directly.
    pub fn forage(&self, index: usize) -> f32 {
        self.grass[index] + self.shrubs[index]
    }

    pub fn food_capacity(&self, index: usize) -> f32 {
        self.capacity(index) + self.shrub_cap[index]
    }

    // Single fullness signal the patch-leaving decision reads: forage / food_capacity.
    pub fn food_frac(&self, index: usize) -> f32 {
        let cap = self.food_capacity(index);
        if cap > 1e-6 { (self.forage(index) / cap).clamp(0.0, 1.0) } else { 0.0 }
    }

    pub fn total_grass(&self) -> f32 {
        self.grass.iter().sum()
    }

    pub fn total_shrubs(&self) -> f32 {
        self.shrubs.iter().sum()
    }

    pub fn reset(&mut self) {
        *self = Self::new(self.width, self.height);
    }
}

pub struct GridPlugin;

impl Plugin for GridPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Grid::new(GRID_WIDTH, GRID_HEIGHT))
            .init_resource::<GrowthRate>()
            .add_systems(
                FixedUpdate,
                (growth, grow_shrubs)
                    .chain()
                    .run_if(in_state(Sim::Running)),
            );
    }
}

const FRESH_DECAY: f32 = 0.05; // ~20-tick half-life
const FRESH_GAIN: f32 = 8.0;

fn growth(mut grid: ResMut<Grid>, rate: Res<GrowthRate>) {
    let n = grid.len();
    let width = grid.width();
    let height = grid.height();
    let caps: Vec<f32> = (0..n).map(|i| grid.capacity(i)).collect();

    let prev_grass = grid.grass.clone();
    let next_grass = field::spread_grow(&prev_grass, &caps, width, height, rate.intrinsic, rate.spread);
    let grew: Vec<f32> = (0..n)
        .map(|i| (next_grass[i] - prev_grass[i]).max(0.0))
        .collect();
    grid.freshness = field::step_freshness(&grid.freshness, &grew, FRESH_DECAY, FRESH_GAIN);
    grid.grass = next_grass;
}

fn grow_shrubs(mut grid: ResMut<Grid>, rate: Res<GrowthRate>) {
    grid.shrubs = field::spread_grow(
        &grid.shrubs,
        &grid.shrub_cap,
        grid.width(),
        grid.height(),
        rate.shrub,
        0.0,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

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
        grid.set_grass(0, 0.8);
        grid.add_poop(1, 0.5);
        grid.set_water(2, 1.0);
        grid.set_water_prox(3, 0.3);
        grid.set_soil(4, 0.2);

        grid.reset();

        assert!((grid.total_grass()).abs() < 1e-6);
        assert!((grid.poop(1)).abs() < 1e-6);
        assert!((grid.water(2)).abs() < 1e-6);
        assert!((grid.water_prox(3) - 1.0).abs() < 1e-6);
        assert!((grid.soil(4) - 1.0).abs() < 1e-6);
        assert_eq!(grid.width(), 4);
        assert_eq!(grid.height(), 4);
    }
}

