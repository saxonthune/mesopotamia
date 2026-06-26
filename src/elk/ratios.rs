use bevy::prelude::*;

use crate::grid::GrowthRate;
use super::components::{ElkParams, BITE, ENERGY_DRAIN};

/// Economy sliders: each governs one regime and cannot disturb the others.
#[derive(Resource, Clone, Copy)]
pub struct RatioControls {
    /// Ticks of life one bite buys; drives `graze_yield` via `apply_ratios`.
    pub feed_ratio: f32,
    pub grass_regrow: f32,
    pub shrub_regrow: f32,
    /// No longer steers crossing; survives as the score pull-penalty knob.
    pub cross_ratio: f32,
}

impl Default for RatioControls {
    fn default() -> Self {
        Self {
            // Break-even at ~62% grazing duty cycle — hungry enough to migrate, not enough to coast.
            feed_ratio: 8.0,
            grass_regrow: 0.02,
            shrub_regrow: 0.0025,
            cross_ratio: 0.0,
        }
    }
}

/// `graze_yield = feed_ratio · ENERGY_DRAIN / BITE`
pub fn graze_yield_from(feed_ratio: f32) -> f32 {
    feed_ratio * ENERGY_DRAIN / BITE
}

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

    #[test]
    fn feed_ratio_is_ticks_of_life_per_bite() {
        for r in [1.0_f32, 2.5, 5.0] {
            let yield_ = graze_yield_from(r);
            let bite_energy = yield_ * BITE;
            assert!((bite_energy / ENERGY_DRAIN - r).abs() < 1e-5, "feed_ratio {r}");
        }
    }
}
