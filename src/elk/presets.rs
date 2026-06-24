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
//! The `HIGH_SCORE` values are **provisional**: with the green wave not yet
//! carrying the herd on the natural drive, the score's optimum is still the
//! degenerate "max pull + max scarcity". Once the green-wave / pull-penalty fix
//! lands, re-sweep and replace it with the low-pull, skill-tuned values.

use crate::grid::GreenWave;
use super::components::ElkParams;
use super::ratios::RatioControls;

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

/// Default regime keeps every drive weight at the resource default.
fn default_params(_p: &mut ElkParams) {}

/// Crossing regime: lean hard on forage, focus the steps, and pay to ford.
fn crossing_params(p: &mut ElkParams) {
    p.grass = 2.0;
    p.temperature = 0.35;
    p.cross = 3.0;
}

/// High-score regime (provisional): a strong forager that, under scarcity, rides
/// the migration pull across — the current degenerate optimum.
fn high_score_params(p: &mut ElkParams) {
    p.grass = 2.5;
    p.social = 2.0;
    p.grass_radius = 10.0;
    p.temperature = 0.4;
}

// ── The table ─────────────────────────────────────────────────────────────────
//
// GreenWave / RatioControls literals are spelled out (not `..default()`) so the
// table is `const` and every value is visible in one place. Keep the DEFAULT rows
// in sync with the resources' own `Default` impls.

/// The three reference regimes, in display order.
pub const PRESETS: [Preset; 3] = [
    Preset {
        name: "Default",
        description: "Stock weights, weak pull, plenty of food — the herd mills and \
                      overcrowds the first segment instead of crossing.",
        ratios: RatioControls { bite_ratio: 2.5, regrow_ratio: 0.175, cross_ratio: 0.35 },
        green_wave: GreenWave { strength: 0.5, speed: 0.010, wavelength: 85.0 },
        apply_params: default_params,
    },
    Preset {
        name: "Crossing",
        description: "Tuned foraging + a real pull under mild scarcity — the herd \
                      visibly crosses the map (the \"it works\" baseline).",
        ratios: RatioControls { bite_ratio: 2.5, regrow_ratio: 0.12, cross_ratio: 1.0 },
        green_wave: GreenWave { strength: 1.0, speed: 0.010, wavelength: 85.0 },
        apply_params: crossing_params,
    },
    Preset {
        name: "High score",
        description: "Provisional: hard scarcity + a strong pull drives the score into \
                      the hundreds — the degenerate optimum, pending the green-wave fix.",
        ratios: RatioControls { bite_ratio: 2.5, regrow_ratio: 0.04, cross_ratio: 2.0 },
        green_wave: GreenWave { strength: 0.0, speed: 0.010, wavelength: 85.0 },
        apply_params: high_score_params,
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
        let crossing = &PRESETS[1];
        let mut ratios = RatioControls::default();
        let mut wave = GreenWave::default();
        let mut params = ElkParams::default();

        apply(crossing, &mut ratios, &mut wave, &mut params);

        assert_eq!(ratios.cross_ratio, 1.0);
        assert_eq!(ratios.regrow_ratio, 0.12);
        assert_eq!(wave.strength, 1.0);
        assert_eq!(params.grass, 2.0, "crossing preset raises the grass weight");
        assert_eq!(params.cross, 3.0);
    }
}
