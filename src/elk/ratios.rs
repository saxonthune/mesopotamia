use bevy::prelude::*;

use crate::grid::GrowthRate;
use super::components::{ElkParams, BITE, ENERGY_DRAIN};

/// The three economy sliders, plus the orthogonal scoring knob. Each of the three
/// governs exactly one regime and cannot disturb the others:
///   - `feed_ratio` (elk) sets how much energy a bite of food yields;
///   - `grass_regrow` (grass) sets grass abundance;
///   - `shrub_regrow` (shrub) sets shrub abundance.
/// `apply_ratios` writes each onto the live resource it drives. The shared mechanics
/// they ride against — `BITE`, `GRAZE_FLOOR`, `ENERGY_DRAIN` — are fixed constants,
/// so turning one slider moves only its own regime.
#[derive(Resource, Clone, Copy)]
pub struct RatioControls {
    /// Elk slider: energy a bite yields ÷ drain per tick — equivalently, ticks of
    /// life one bite buys. Sets the shared `graze_yield` (grass and shrub alike).
    pub feed_ratio: f32,
    /// Grass slider: grass regrowth per tick (the logistic `intrinsic`).
    pub grass_regrow: f32,
    /// Shrub slider: shrub regrowth per tick.
    pub shrub_regrow: f32,
    /// Scoring penalty knob, in [0, 1] — orthogonal to food. The herd fords on the
    /// natural drives, so this no longer steers; it survives only as the score
    /// penalty (`score::pull_factor`). Default 0.0 → no penalty.
    pub cross_ratio: f32,
}

impl Default for RatioControls {
    fn default() -> Self {
        Self {
            // 8 ticks of life per bite, one bite per CHEW_TICKS+1 ticks → break-even
            // when grazing ~62% of ticks: mid-fed, hungry enough to keep grazing but
            // with slack to travel, never the overfed coast of the old 2.5.
            feed_ratio: 8.0,
            grass_regrow: 0.02,
            shrub_regrow: 0.0025,
            cross_ratio: 0.0,
        }
    }
}

/// `feed_ratio = bite_energy / drain = BITE · graze_yield / ENERGY_DRAIN`
/// ⇒ `graze_yield = feed_ratio · ENERGY_DRAIN / BITE`
pub fn graze_yield_from(feed_ratio: f32) -> f32 {
    feed_ratio * ENERGY_DRAIN / BITE
}

/// Bevy system: pushes the economy sliders onto the live resources each tick — the
/// shared `graze_yield` onto `ElkParams`, and grass/shrub regrowth onto `GrowthRate`.
pub fn apply_ratios(
    controls: Res<RatioControls>,
    mut elk: ResMut<ElkParams>,
    mut growth: ResMut<GrowthRate>,
) {
    elk.graze_yield = graze_yield_from(controls.feed_ratio);
    growth.intrinsic = controls.grass_regrow;
    growth.shrub = controls.shrub_regrow;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feed_ratio_sets_yield() {
        // feed_ratio 2.5 → 2.5 · 0.002 / 0.05 = 0.1 energy per food unit.
        assert!((graze_yield_from(2.5) - 0.1).abs() < 1e-6);
    }

    // The elk slider is the inverse break-even duty cycle: a bite buys `feed_ratio`
    // ticks of drain, so the herd must graze 1/feed_ratio of the time to hold even.
    #[test]
    fn feed_ratio_is_ticks_of_life_per_bite() {
        for r in [1.0_f32, 2.5, 5.0] {
            let yield_ = graze_yield_from(r);
            let bite_energy = yield_ * BITE;
            assert!((bite_energy / ENERGY_DRAIN - r).abs() < 1e-5, "feed_ratio {r}");
        }
    }
}
