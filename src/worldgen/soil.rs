//! Soil layer: static fertility = fine low-frequency patches × a coarse regional
//! macro-octave. The product breaks the smooth water-distance gradient into rich
//! thickets and poor scrapes, with broad lush/sparse zones layered over them.
//! Authored once, before the sim runs; `Grid::capacity` inherits the variation.

use rand::SeedableRng;
use rand::rngs::StdRng;

use crate::field;
use crate::grid::Grid;

// Fine soil patches. A distinct seed from the river so the two heterogeneities
// (where water is vs. where the ground is rich) are uncorrelated.
const SOIL_SEED: u64 = 0x501;
const SOIL_PASSES: usize = 6; // smoothing passes — higher = broader patches
const SOIL_FLOOR: f32 = 0.35; // the poorest ground still carries this fraction

// Macro (coarse) regional fertility octave — broad lush vs. sparse zones. A
// distinct seed so coarse regions are uncorrelated with the fine patches.
const MACRO_SEED: u64 = 0xA7C3;
const MACRO_PASSES: usize = 14; // many passes → features span large fractions of the map
const MACRO_FLOOR: f32 = 0.4; // the sparsest region still carries this fraction

/// Author the static patchiness field: fine value noise (soil patches) multiplied
/// by a coarse macro-octave (regional lush/sparse zones). Both factors are in
/// (0, 1] so `soil` stays in [0, 1]; `capacity` inherits the regional variation.
pub(super) fn seed_soil(grid: &mut Grid) {
    let mut rng = StdRng::seed_from_u64(SOIL_SEED);
    let noise = field::value_noise(grid.width(), grid.height(), SOIL_PASSES, &mut rng);
    let patch = field::normalize(&noise);

    let mut macro_rng = StdRng::seed_from_u64(MACRO_SEED);
    let macro_noise = field::value_noise(grid.width(), grid.height(), MACRO_PASSES, &mut macro_rng);
    let macro_patch = field::normalize(&macro_noise);

    for (index, (&p, &m)) in patch.iter().zip(macro_patch.iter()).enumerate() {
        let fine = SOIL_FLOOR + (1.0 - SOIL_FLOOR) * p;
        let region = MACRO_FLOOR + (1.0 - MACRO_FLOOR) * m;
        grid.set_soil(index, fine * region);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::{Grid, GRID_HEIGHT, GRID_WIDTH};

    /// `seed_soil` must produce a field with genuine regional spread — both the
    /// macro octave and the fine patches are blended in, so the resulting soil has
    /// a wide cell-level range and is bounded within [0, 1]. Fixed seeds → deterministic.
    #[test]
    fn soil_has_regional_spread() {
        let mut grid = Grid::new(GRID_WIDTH, GRID_HEIGHT);
        seed_soil(&mut grid);

        let n = grid.len();
        let min: f32 = (0..n).map(|i| grid.soil(i)).fold(f32::MAX, f32::min);
        let max: f32 = (0..n).map(|i| grid.soil(i)).fold(f32::MIN, f32::max);

        println!("soil range: min={min:.4} max={max:.4} spread={:.4}", max - min);

        // Values must stay in [0, 1] (clamped by set_soil).
        assert!(min >= 0.0 && max <= 1.0, "soil out of [0,1]: min={min} max={max}");

        // The macro octave must produce substantial regional contrast: with fine
        // floor 0.35 and macro floor 0.4 the theoretical range is [0.14, 1.0]; in
        // practice the normalised noise uses the full [0,1] range, so we expect a
        // cell-level spread of at least 0.3.
        assert!(
            max - min > 0.3,
            "regional spread too narrow: soil range = {:.4} (min={min:.4} max={max:.4})",
            max - min
        );
    }
}
