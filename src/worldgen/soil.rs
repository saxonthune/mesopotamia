//! Soil layer: per-tile grain × fine patches × coarse macro-octave, authored once before the sim.
//! The three scales break the water-distance gradient into stippled fertility rather than clean bands.

use rand::SeedableRng;
use rand::rngs::StdRng;

use crate::field;
use crate::grid::Grid;

const SOIL_PASSES: usize = 6;  // higher → broader patches
const SOIL_FLOOR: f32 = 0.35;
const MACRO_PASSES: usize = 14; // many passes → features span large map fractions
const MACRO_FLOOR: f32 = 0.4;
const GRAIN_PASSES: usize = 0; // 0 → independent per-tile variation (no smoothing)
const GRAIN_FLOOR: f32 = 0.5;
/// XOR salt so the grain octave draws an independent RNG stream from the fine patches.
const GRAIN_SALT: u64 = 0x6_4A12;

/// `soil_seed` and `macro_seed` must differ from each other and from the river seed (uncorrelated heterogeneities).
pub(super) fn seed_soil(grid: &mut Grid, soil_seed: u64, macro_seed: u64) {
    let mut rng = StdRng::seed_from_u64(soil_seed);
    let noise = field::value_noise(grid.width(), grid.height(), SOIL_PASSES, &mut rng);
    let patch = field::normalize(&noise);

    let mut macro_rng = StdRng::seed_from_u64(macro_seed);
    let macro_noise = field::value_noise(grid.width(), grid.height(), MACRO_PASSES, &mut macro_rng);
    let macro_patch = field::normalize(&macro_noise);

    let mut grain_rng = StdRng::seed_from_u64(soil_seed ^ GRAIN_SALT);
    let grain_noise = field::value_noise(grid.width(), grid.height(), GRAIN_PASSES, &mut grain_rng);
    let grain = field::normalize(&grain_noise);

    for (index, ((&p, &m), &g)) in patch.iter().zip(macro_patch.iter()).zip(grain.iter()).enumerate() {
        let fine = SOIL_FLOOR + (1.0 - SOIL_FLOOR) * p;
        let region = MACRO_FLOOR + (1.0 - MACRO_FLOOR) * m;
        let tile = GRAIN_FLOOR + (1.0 - GRAIN_FLOOR) * g;
        grid.set_soil(index, fine * region * tile);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::{Grid, GRID_HEIGHT, GRID_WIDTH};

    #[test]
    fn soil_has_regional_spread() {
        let mut grid = Grid::new(GRID_WIDTH, GRID_HEIGHT);
        seed_soil(&mut grid, 0x501, 0xA7C3);

        let n = grid.len();
        let min: f32 = (0..n).map(|i| grid.soil(i)).fold(f32::MAX, f32::min);
        let max: f32 = (0..n).map(|i| grid.soil(i)).fold(f32::MIN, f32::max);

        println!("soil range: min={min:.4} max={max:.4} spread={:.4}", max - min);

        assert!(min >= 0.0 && max <= 1.0, "soil out of [0,1]: min={min} max={max}");
        assert!(
            max - min > 0.3,
            "regional spread too narrow: soil range = {:.4} (min={min:.4} max={max:.4})",
            max - min
        );
    }

    #[test]
    fn grain_roughens_neighbours() {
        let mut grid = Grid::new(GRID_WIDTH, GRID_HEIGHT);
        seed_soil(&mut grid, 0x501, 0xA7C3);

        let mut sum = 0.0f32;
        let mut count = 0u32;
        for row in 0..GRID_HEIGHT {
            for col in 0..GRID_WIDTH - 1 {
                let i = row * GRID_WIDTH + col;
                sum += (grid.soil(i) - grid.soil(i + 1)).abs();
                count += 1;
            }
        }
        let mean_neighbour_diff = sum / count as f32;
        println!("mean adjacent soil diff = {mean_neighbour_diff:.4}");
        assert!(
            mean_neighbour_diff > 0.05,
            "per-tile grain too weak: adjacent cells differ by only {mean_neighbour_diff:.4} on average"
        );
    }
}
