//! Rough-terrain layer: noise-thresholded patches of broken ground. Dense noise
//! clumps settle on dry ground far from water and are excluded from water cells —
//! the same dryness-gated, clump-forming placement vegetation uses. Reads the
//! water field the water layer already laid down, which is why it runs after it.
//!
//! This is the directional-grain / wisp feature in its placeholder form: the
//! orientation/vector-field and void-aware spacing the methodology calls for are
//! not in scope here. The per-cell decision is kept pure and small so the
//! placement internals can be swapped later without disturbing the seam.

use rand::SeedableRng;
use rand::rngs::StdRng;

use crate::field;
use crate::grid::{Grid, MAX_ROUGH};

const ROUGH_PASSES: usize = 5;
const ROUGH_THRESHOLD: f32 = 0.6; // only the densest noise becomes a rough patch

/// Per-cell rough-terrain decision, extracted pure so the placement contract is
/// pinned by tests rather than read off the screen.
///
/// - Zero in any water cell (`water > 0`).
/// - Zero below the noise `threshold` so patches clump rather than wash uniformly.
/// - Otherwise intensity scales with `dryness` (1 = far from water) up to `max`.
fn rough_at(dryness: f32, noise: f32, water: f32, threshold: f32, max: f32) -> f32 {
    if water > 0.0 || noise < threshold {
        0.0
    } else {
        dryness * max
    }
}

/// Author rough-terrain presence: dense noise clumps on dry ground (far from
/// water), excluded from water cells. Reads `water`/`water_prox` straight off the
/// grid, which the water layer has already filled by the time this layer runs.
///
/// `rough_seed` is decorrelated from the soil, river, and shrub seeds so the
/// patches fall independently of the other layers; the orchestrator derives it
/// from the master world seed.
pub(super) fn seed_rough(grid: &mut Grid, rough_seed: u64) {
    let mut rng = StdRng::seed_from_u64(rough_seed);
    let patch = field::normalize(&field::value_noise(
        grid.width(),
        grid.height(),
        ROUGH_PASSES,
        &mut rng,
    ));
    for (i, &p) in patch.iter().enumerate() {
        // Dryness: far from water → 1, beside it → 0. Rough ground wants the steppe.
        let dryness = 1.0 - grid.water_prox(i);
        let r = rough_at(dryness, p, grid.water(i), ROUGH_THRESHOLD, MAX_ROUGH);
        grid.set_rough(i, r);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_in_water() {
        // Even a dry, above-threshold cell is zero if it holds water.
        assert_eq!(rough_at(1.0, 1.0, 0.5, ROUGH_THRESHOLD, MAX_ROUGH), 0.0);
    }

    #[test]
    fn zero_below_threshold() {
        let n = ROUGH_THRESHOLD - 0.01;
        assert_eq!(rough_at(1.0, n, 0.0, ROUGH_THRESHOLD, MAX_ROUGH), 0.0);
    }

    #[test]
    fn intensity_rises_with_dryness() {
        // Above threshold, on dry land: intensity grows with dryness.
        let wet = rough_at(0.2, 1.0, 0.0, ROUGH_THRESHOLD, MAX_ROUGH);
        let dry = rough_at(0.9, 1.0, 0.0, ROUGH_THRESHOLD, MAX_ROUGH);
        assert!(dry > wet, "drier ground should carry rougher terrain");
    }
}
