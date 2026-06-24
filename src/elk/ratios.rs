use bevy::prelude::*;

use crate::grid::{GrowthRate, MAX_GRASS};
use super::ElkParams;

/// Dimensionless ratio sliders. Each ratio governs one behavioural regime;
/// the corresponding absolute parameter is derived each tick.
///
/// Defaults are the exact inverses of the documented default magnitudes so
/// `apply_ratios` reproduces them at startup without any behavioural change.
#[derive(Resource, Clone, Copy)]
pub struct RatioControls {
    /// intake-per-bite ÷ drain: `bite * graze_yield / energy_drain`.
    /// Default 4.375 → graze_yield = 0.035.
    pub bite_ratio: f32,
    /// regrowth ÷ drain: `intrinsic * MAX_GRASS * graze_yield / energy_drain`.
    /// Default 0.175 → GrowthRate.intrinsic = 0.02.
    pub regrow_ratio: f32,
    /// migration ÷ crossing cost: `migration / water_cost`.
    /// Default 0.35 → ElkParams.migration = 0.7.
    pub cross_ratio: f32,
}

impl Default for RatioControls {
    fn default() -> Self {
        // Computed from documented defaults:
        //   bite=0.5, graze_yield=0.035, energy_drain=0.004 → 0.5*0.035/0.004 = 4.375
        //   intrinsic=0.02, MAX_GRASS=1.0, graze_yield=0.035, energy_drain=0.004 → 0.175
        //   migration=0.7, water_cost=2.0 → 0.35
        Self {
            // Tightened from 4.375 (which left elk overfed — break-even at 23% of
            // ticks, so a full herd just coasts and sprints past grass). At 2.5 the
            // break-even is ~40%: the herd settles mid-fed (energy ~0.55), hungry
            // enough to keep grazing rather than coast, yet able to sustain itself
            // when it forages — the stakes that make the tuning puzzle real.
            bite_ratio: 2.5,
            regrow_ratio: 0.175,
            cross_ratio: 0.35,
        }
    }
}

/// `bite_ratio = bite * graze_yield / energy_drain`
/// ⇒ `graze_yield = bite_ratio * energy_drain / bite`
pub fn graze_yield_from(bite_ratio: f32, bite: f32, energy_drain: f32) -> f32 {
    bite_ratio * energy_drain / bite
}

/// `regrow_ratio = intrinsic * cap_ref * graze_yield / energy_drain`
/// ⇒ `intrinsic = regrow_ratio * energy_drain / (cap_ref * graze_yield)`
pub fn intrinsic_from(regrow_ratio: f32, energy_drain: f32, graze_yield: f32, cap_ref: f32) -> f32 {
    regrow_ratio * energy_drain / (cap_ref * graze_yield)
}

/// `cross_ratio = migration / water_cost`
/// ⇒ `migration = cross_ratio * water_cost`
pub fn migration_from(cross_ratio: f32, water_cost: f32) -> f32 {
    cross_ratio * water_cost
}

/// Bevy system: reads `RatioControls` and anchor params, writes the three
/// derived values into `ElkParams` and `GrowthRate`. Derive in dependency
/// order: graze_yield first (needed by regrowth ratio), then intrinsic,
/// then migration.
pub fn apply_ratios(
    controls: Res<RatioControls>,
    mut elk: ResMut<ElkParams>,
    mut growth: ResMut<GrowthRate>,
) {
    if elk.bite > 0.0 {
        elk.graze_yield = graze_yield_from(controls.bite_ratio, elk.bite, elk.energy_drain);
    }
    let gy = elk.graze_yield;
    if gy > 0.0 {
        growth.intrinsic = intrinsic_from(controls.regrow_ratio, elk.energy_drain, gy, MAX_GRASS);
    }
    elk.migration = migration_from(controls.cross_ratio, elk.water_cost);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_derive_expected_magnitudes() {
        let rc = RatioControls::default();
        let bite = 0.5_f32;
        let energy_drain = 0.004_f32;
        let water_cost = 2.0_f32;

        // bite_ratio 2.5, bite 0.5, drain 0.004 → 2.5*0.004/0.5 = 0.02.
        let graze_yield = graze_yield_from(rc.bite_ratio, bite, energy_drain);
        assert!((graze_yield - 0.02).abs() < 1e-6, "graze_yield {graze_yield}");

        // regrow_ratio 0.175 holds regrowth at 0.175× drain regardless of yield:
        // 0.175*0.004/(1.0*0.02) = 0.035.
        let intrinsic = intrinsic_from(rc.regrow_ratio, energy_drain, graze_yield, MAX_GRASS);
        assert!((intrinsic - 0.035).abs() < 1e-6, "intrinsic {intrinsic}");

        let migration = migration_from(rc.cross_ratio, water_cost);
        assert!((migration - 0.7).abs() < 1e-6, "migration {migration}");
    }

    #[test]
    fn bite_ratio_round_trips() {
        let bite = 0.5_f32;
        let energy_drain = 0.004_f32;
        for r in [1.0_f32, 4.375, 8.0] {
            let gy = graze_yield_from(r, bite, energy_drain);
            let back = gy * bite / energy_drain;
            assert!((back - r).abs() < 1e-5, "round-trip {r} → {back}");
        }
    }

    #[test]
    fn regrow_ratio_round_trips() {
        let energy_drain = 0.004_f32;
        let graze_yield = 0.035_f32;
        for r in [0.1_f32, 0.175, 0.5] {
            let intr = intrinsic_from(r, energy_drain, graze_yield, MAX_GRASS);
            let back = intr * MAX_GRASS * graze_yield / energy_drain;
            assert!((back - r).abs() < 1e-5, "round-trip {r} → {back}");
        }
    }

    #[test]
    fn cross_ratio_round_trips() {
        let water_cost = 2.0_f32;
        for r in [0.2_f32, 0.35, 1.0] {
            let mig = migration_from(r, water_cost);
            let back = mig / water_cost;
            assert!((back - r).abs() < 1e-5, "round-trip {r} → {back}");
        }
    }
}
