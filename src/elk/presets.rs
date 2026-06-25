//! Named slider presets — the three reference regimes the demo is built around,
//! as one editable data table.
//!
//! A [`Preset`] bundles the knobs a player tunes — the [`RatioControls`] economy
//! ratios, the [`GreenWave`] crest, and the drive-weight overrides on top of
//! [`ElkParams::default`] — under a name. [`apply`] writes a preset onto the live
//! resources (the UI's preset buttons), and the harness re-runs the same bundles
//! to verify each still meets its goal (see the `preset_*` checks in
//! `tests/herd_shape.rs`).
//!
//! **Updating a preset is a one-line edit here.** The values come from the
//! `evaluate_bundle` sweeps in the harness; when the simulation changes, re-run
//! those sweeps and edit the table below — nothing else moves.
//!
//! These are the **working set** the current model is tuned around: `Default` is the
//! broken stage-1 herd (resource defaults — mills and overcrowds, no green-wave
//! following), `Can cross` is the forage-sticky regime that rolls and fords the river
//! on the *natural* drives (the green wave, not the pull), and `Optimized` is the same
//! movement under leaner scarcity — a stage-3 score target. `Can cross`/`Optimized`
//! run at **zero pull**: the crossing is earned by chasing the green-up front across
//! the water, which the bounded swim cost makes passable.

use crate::grid::GreenWave;
use super::components::ElkParams;
use super::ratios::RatioControls;

/// The forage-sticky movement the working presets share: the herd follows the
/// green-up front (`freshness_weight`), leapfrogs forward off depleted ground
/// (`sightline_*`), forms a rolling column (`cohesion_lead`), and moves as a sticky
/// glob (`momentum`/`temperature`). Survival on the long march to the river comes
/// from the slow-chew + low-drain standard physiology baked into `ElkParams::default`
/// — these overrides are the *movement* the player tunes on top of it. Economy
/// (bite_ratio/regrow) and the green wave are set per preset.
fn forage_sticky(p: &mut ElkParams) {
    p.grass = 2.0;
    p.grass_radius = 8.0;
    p.freshness_weight = 4.0;
    p.sightline_range = 24.0;
    p.sightline_weight = 3.0;
    p.cohesion_lead = 1.0;
    p.momentum = 0.5;
    p.temperature = 0.4;
}

/// One named regime: the full set of tunables that define it.
pub struct Preset {
    /// Short label shown on the UI button.
    pub name: &'static str,
    /// One-line hover description of what the regime is for.
    pub description: &'static str,
    /// Economy ratios (bite / regrow / cross).
    pub ratios: RatioControls,
    /// The travelling green-up crest.
    pub green_wave: GreenWave,
    /// Drive-weight / perception overrides applied on top of `ElkParams::default()`.
    /// A named `fn` (not a closure) so the table is `const`.
    pub apply_params: fn(&mut ElkParams),
}

// ── Per-preset ElkParams overrides ────────────────────────────────────────────

/// Default regime keeps every drive weight at the resource default — the broken
/// stage-1 herd that mills, overcrowds, and never follows the green wave.
fn default_params(_p: &mut ElkParams) {}

/// Can-cross regime: the forage-sticky movement, nothing added — it rolls with the
/// green-up front and fords the river on the natural drives at zero pull.
fn can_cross_params(p: &mut ElkParams) {
    forage_sticky(p);
}

/// Optimized regime: same forage-sticky movement, tuned a touch harder (sharper
/// pick, longer sight) for a herd that has to hold together under scarcity.
fn optimized_params(p: &mut ElkParams) {
    forage_sticky(p);
    p.temperature = 0.35;
    p.sightline_range = 28.0;
}

// ── The table ─────────────────────────────────────────────────────────────────
//
// GreenWave / RatioControls literals are spelled out (not `..default()`) so the
// table is `const` and every value is visible in one place. Keep the DEFAULT rows
// in sync with the resources' own `Default` impls.

/// The three working regimes, in display order.
pub const PRESETS: [Preset; 3] = [
    Preset {
        name: "Default",
        description: "Stock weights, no pull, plenty of food — with no eastward drive the \
                      herd mills and overcrowds the first segment, never crossing (stage 1, broken).",
        ratios: RatioControls { bite_ratio: 2.5, regrow_ratio: 0.175, cross_ratio: 0.0 },
        // Green wave unplugged (strength 0) to match GreenWave::default() — see grid.rs.
        green_wave: GreenWave { strength: 0.0, speed: 0.010, wavelength: 85.0 },
        apply_params: default_params,
    },
    Preset {
        name: "Can cross",
        description: "Forage-sticky: follows the green-up front, rolls as a glob, and \
                      fords the river on the natural drives at zero pull — easy economy.",
        ratios: RatioControls { bite_ratio: 4.0, regrow_ratio: 0.25, cross_ratio: 0.0 },
        green_wave: GreenWave { strength: 0.4, speed: 0.004, wavelength: 70.0 },
        apply_params: can_cross_params,
    },
    Preset {
        name: "Optimized",
        description: "The same natural-drive crossing under leaner scarcity — a stage-3 \
                      score target where the model must hold together as food thins.",
        ratios: RatioControls { bite_ratio: 2.5, regrow_ratio: 0.12, cross_ratio: 0.0 },
        green_wave: GreenWave { strength: 0.4, speed: 0.004, wavelength: 70.0 },
        apply_params: optimized_params,
    },
];

/// Write a preset onto the live resources. Resets `ElkParams` to default first so
/// the preset is a *complete*, reproducible configuration (no leftover from a
/// previous preset), then applies its overrides. The derived economy params
/// (`graze_yield`/`intrinsic`/`migration`) are recomputed from `ratios` by
/// `apply_ratios` each tick, so they are not set here.
pub fn apply(
    preset: &Preset,
    ratios: &mut RatioControls,
    wave: &mut GreenWave,
    params: &mut ElkParams,
) {
    *ratios = preset.ratios;
    *wave = preset.green_wave;
    *params = ElkParams::default();
    (preset.apply_params)(params);
}

#[cfg(test)]
mod tests {
    use super::*;

    // Every preset has a non-empty, unique name (the UI buttons key off them).
    #[test]
    fn presets_have_unique_nonempty_names() {
        for p in &PRESETS {
            assert!(!p.name.is_empty(), "preset name must not be empty");
        }
        for (i, a) in PRESETS.iter().enumerate() {
            for b in &PRESETS[i + 1..] {
                assert_ne!(a.name, b.name, "preset names must be unique");
            }
        }
    }

    // The Default preset reproduces the resources' own defaults — applying it is a
    // true "reset to stock", so its table rows must track the Default impls.
    #[test]
    fn default_preset_reproduces_resource_defaults() {
        let mut ratios = RatioControls { bite_ratio: 0.0, regrow_ratio: 0.0, cross_ratio: 0.0 };
        let mut wave = GreenWave { strength: -1.0, speed: -1.0, wavelength: -1.0 };
        let mut params = ElkParams::default();
        params.grass = 999.0; // dirty it to prove apply() resets

        apply(&PRESETS[0], &mut ratios, &mut wave, &mut params);

        let rd = RatioControls::default();
        assert_eq!(ratios.bite_ratio, rd.bite_ratio);
        assert_eq!(ratios.regrow_ratio, rd.regrow_ratio);
        assert_eq!(ratios.cross_ratio, rd.cross_ratio);
        let wd = GreenWave::default();
        assert_eq!(wave.strength, wd.strength);
        assert_eq!(wave.speed, wd.speed);
        assert_eq!(wave.wavelength, wd.wavelength);
        assert_eq!(params.grass, ElkParams::default().grass, "apply() must reset ElkParams");
    }

    // Applying a non-default preset writes its bundle onto the resources.
    #[test]
    fn apply_writes_the_bundle() {
        let can_cross = &PRESETS[1];
        let mut ratios = RatioControls::default();
        let mut wave = GreenWave::default();
        let mut params = ElkParams::default();

        apply(can_cross, &mut ratios, &mut wave, &mut params);

        assert_eq!(ratios.cross_ratio, 0.0, "can-cross fords on natural drives, not the pull");
        assert_eq!(ratios.regrow_ratio, 0.25);
        assert_eq!(wave.strength, 0.4);
        assert_eq!(params.grass, 2.0, "can-cross raises the grass weight");
        assert_eq!(params.freshness_weight, 4.0, "can-cross follows the green-up front");
        assert_eq!(params.sightline_weight, 3.0, "can-cross leapfrogs forward");
    }
}
