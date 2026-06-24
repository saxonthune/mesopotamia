//! Soil layer: static fertility across three scales — a per-tile grain × fine
//! low-frequency patches × a coarse regional macro-octave. The product breaks the
//! smooth water-distance gradient into rich thickets and poor scrapes, with broad
//! lush/sparse zones layered over them, and the per-tile grain roughens the result
//! so the discrete water-proximity ranks read as a stippled mat rather than clean
//! concentric bands of grass (Alexander's roughness). Authored once, before the sim
//! runs; `Grid::capacity` — which multiplies `soil` by water proximity — inherits
//! all three scales, and only grass reads it (the dirt tint rides water proximity).

use rand::SeedableRng;
use rand::rngs::StdRng;

use crate::field;
use crate::grid::Grid;

// Fine soil patches.
const SOIL_PASSES: usize = 6; // smoothing passes — higher = broader patches
const SOIL_FLOOR: f32 = 0.35; // the poorest ground still carries this fraction

// Macro (coarse) regional fertility octave — broad lush vs. sparse zones.
const MACRO_PASSES: usize = 14; // many passes → features span large fractions of the map
const MACRO_FLOOR: f32 = 0.4; // the sparsest region still carries this fraction

// Per-tile grain — high-frequency fertility roughness that dithers the discrete
// water-proximity ranks so grass reads as a rough mat, not clean concentric bands.
const GRAIN_PASSES: usize = 0; // no smoothing → independent per-tile variation
const GRAIN_FLOOR: f32 = 0.5; // the thinnest-grained tile still carries half
/// Salt mixed into `soil_seed` for the grain RNG so the grain octave draws an
/// independent stream from the fine patches and is not correlated with them.
const GRAIN_SALT: u64 = 0x6_4A12;

/// Author the static patchiness field: fine value noise (soil patches) multiplied
/// by a coarse macro-octave (regional lush/sparse zones). Both factors are in
/// (0, 1] so `soil` stays in [0, 1]; `capacity` inherits the regional variation.
///
/// `soil_seed` and `macro_seed` must differ from each other and from the river
/// seed so the three heterogeneities (water, fine patches, regional zones) stay
/// uncorrelated; the orchestrator derives both from the master world seed.
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

    /// `seed_soil` must produce a field with genuine regional spread — both the
    /// macro octave and the fine patches are blended in, so the resulting soil has
    /// a wide cell-level range and is bounded within [0, 1]. Fixed seeds → deterministic.
    #[test]
    fn soil_has_regional_spread() {
        let mut grid = Grid::new(GRID_WIDTH, GRID_HEIGHT);
        // Two distinct fixed seeds keep the test deterministic and the fine/macro
        // octaves uncorrelated, mirroring what the orchestrator derives at runtime.
        seed_soil(&mut grid, 0x501, 0xA7C3);

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

    /// The per-tile grain must roughen the field at the *cell* scale, not just
    /// regionally: neighbouring cells differ enough that a smooth water-distance
    /// rank dithers into a stippled mat rather than reading as one clean band. We
    /// measure the mean absolute difference between horizontally adjacent cells —
    /// a broad-only field (patches × region) leaves this near zero.
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
