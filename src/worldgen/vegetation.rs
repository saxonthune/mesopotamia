//! Vegetation layer: shrub (big-leaf) carrying capacity. Dense noise clumps
//! settle on dry ground far from water and are excluded from water cells. Reads the
//! water field the water layer already laid down — which is exactly why it runs
//! after it in the orchestrator. The runtime shrub regrowth (in `grid`) grows the
//! standing crop toward this capacity.

use rand::SeedableRng;
use rand::rngs::StdRng;

use crate::field;
use crate::grid::{Grid, MAX_SHRUBS};

// A separate seed and patch from soil/river so the shrub clumps fall independently
// of where water and good grazing land are.
const SHRUB_SEED: u64 = 0xB405E;
const SHRUB_PASSES: usize = 5;
const SHRUB_THRESHOLD: f32 = 0.55; // only the densest noise becomes a shrub clump

/// Author shrub capacity: dense noise clumps on dry ground (far from water),
/// excluded from water cells. Reads `water`/`water_prox` straight off the grid,
/// which the water layer has already filled by the time this layer runs.
pub(super) fn seed_shrub_cap(grid: &mut Grid) {
    let mut rng = StdRng::seed_from_u64(SHRUB_SEED);
    let patch = field::normalize(&field::value_noise(
        grid.width(),
        grid.height(),
        SHRUB_PASSES,
        &mut rng,
    ));
    for (i, &p) in patch.iter().enumerate() {
        // Dryness: far from water → 1, beside it → 0. Shrubs want the steppe.
        let dry = 1.0 - grid.water_prox(i);
        // Gentle nudge: riparian cells (soil_type → 1) slightly suppress shrubs.
        let steppe = 1.0 - 0.5 * grid.soil_type(i);
        let cap = if grid.water(i) > 0.0 || p < SHRUB_THRESHOLD {
            0.0
        } else {
            dry * steppe * MAX_SHRUBS
        };
        grid.set_shrub_cap(i, cap);
    }
}
