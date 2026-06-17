//! Vegetation layer: browse (big-leaf shrub) carrying capacity. Dense noise clumps
//! settle on dry ground far from water and are excluded from water cells. Reads the
//! water field the water layer already laid down — which is exactly why it runs
//! after it in the orchestrator. The runtime browse regrowth (in `grid`) grows the
//! standing crop toward this capacity.

use rand::SeedableRng;
use rand::rngs::StdRng;

use crate::field;
use crate::grid::{Grid, MAX_BROWSE};

// A separate seed and patch from soil/river so the shrub clumps fall independently
// of where water and good grazing land are.
const BROWSE_SEED: u64 = 0xB405E;
const BROWSE_PASSES: usize = 5;
const BROWSE_THRESHOLD: f32 = 0.55; // only the densest noise becomes a shrub clump

/// Author browse capacity: dense noise clumps on dry ground (far from water),
/// excluded from water cells. Reads `water`/`water_prox` straight off the grid,
/// which the water layer has already filled by the time this layer runs.
pub(super) fn seed_browse_cap(grid: &mut Grid) {
    let mut rng = StdRng::seed_from_u64(BROWSE_SEED);
    let patch = field::normalize(&field::value_noise(
        grid.width(),
        grid.height(),
        BROWSE_PASSES,
        &mut rng,
    ));
    for (i, &p) in patch.iter().enumerate() {
        // Dryness: far from water → 1, beside it → 0. Browse wants the steppe.
        let dry = 1.0 - grid.water_prox(i);
        // Gentle nudge: riparian cells (soil_type → 1) slightly suppress browse.
        let steppe = 1.0 - 0.5 * grid.soil_type(i);
        let cap = if grid.water(i) > 0.0 || p < BROWSE_THRESHOLD {
            0.0
        } else {
            dry * steppe * MAX_BROWSE
        };
        grid.set_browse_cap(i, cap);
    }
}
