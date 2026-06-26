//! Named slider presets — the three reference regimes as one editable data table.
//! Update a preset by editing this file; values come from the `evaluate_bundle` sweeps.

use crate::grid::GreenWave;
use super::components::ElkParams;
use super::ratios::RatioControls;

/// Shared forage-perception overrides: wider radius, freshness pull, forward sightline.
fn forage_sticky(p: &mut ElkParams) {
    p.grass_radius = 8.0;
    p.freshness_weight = 4.0;
    p.sightline_range = 24.0;
    p.sightline_weight = 3.0;
}

pub struct Preset {
    pub name: &'static str,
    pub description: &'static str,
    pub ratios: RatioControls,
    pub green_wave: GreenWave,
    /// Named fn (not a closure) so the table is `const`.
    pub apply_params: fn(&mut ElkParams),
}

fn default_params(_p: &mut ElkParams) {}

fn can_cross_params(p: &mut ElkParams) {
    forage_sticky(p);
}

fn optimized_params(p: &mut ElkParams) {
    forage_sticky(p);
    p.sightline_range = 28.0;
}

// Literals (not `..default()`) so the table is `const` and every value is visible.
// Keep DEFAULT rows in sync with the resources' own Default impls.
pub const PRESETS: [Preset; 3] = [
    Preset {
        name: "Default",
        description: "Stock weights, no pull, plenty of food — with no eastward drive the \
                      herd mills and overcrowds the first segment, never crossing (stage 1, broken).",
        ratios: RatioControls { feed_ratio: 8.0, grass_regrow: 0.02, shrub_regrow: 0.0025, cross_ratio: 0.0 },
        green_wave: GreenWave { strength: 0.0, speed: 0.010, wavelength: 85.0 },
        apply_params: default_params,
    },
    Preset {
        name: "Can cross",
        description: "Forage-sticky: follows the green-up front, rolls as a glob, and \
                      fords the river on the natural drives at zero pull — easy economy.",
        ratios: RatioControls { feed_ratio: 10.0, grass_regrow: 0.029, shrub_regrow: 0.0025, cross_ratio: 0.0 },
        green_wave: GreenWave { strength: 0.4, speed: 0.004, wavelength: 70.0 },
        apply_params: can_cross_params,
    },
    Preset {
        name: "Optimized",
        description: "The same natural-drive crossing under leaner scarcity — a stage-3 \
                      score target where the model must hold together as food thins.",
        ratios: RatioControls { feed_ratio: 6.0, grass_regrow: 0.014, shrub_regrow: 0.0025, cross_ratio: 0.0 },
        green_wave: GreenWave { strength: 0.4, speed: 0.004, wavelength: 70.0 },
        apply_params: optimized_params,
    },
];

/// Write a preset onto the live resources. Resets `ElkParams` to default first so no
/// leftover from a previous preset bleeds in; derived economy params recomputed by `apply_ratios`.
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

    #[test]
    fn default_preset_reproduces_resource_defaults() {
        let mut ratios = RatioControls { feed_ratio: 0.0, grass_regrow: 0.0, shrub_regrow: 0.0, cross_ratio: 0.0 };
        let mut wave = GreenWave { strength: -1.0, speed: -1.0, wavelength: -1.0 };
        let mut params = ElkParams::default();
        params.grass_radius = 999.0; // dirty it to prove apply() resets

        apply(&PRESETS[0], &mut ratios, &mut wave, &mut params);

        let rd = RatioControls::default();
        assert_eq!(ratios.feed_ratio, rd.feed_ratio);
        assert_eq!(ratios.grass_regrow, rd.grass_regrow);
        assert_eq!(ratios.shrub_regrow, rd.shrub_regrow);
        assert_eq!(ratios.cross_ratio, rd.cross_ratio);
        let wd = GreenWave::default();
        assert_eq!(wave.strength, wd.strength);
        assert_eq!(wave.speed, wd.speed);
        assert_eq!(wave.wavelength, wd.wavelength);
        assert_eq!(params.grass_radius, ElkParams::default().grass_radius, "apply() must reset ElkParams");
    }

    #[test]
    fn apply_writes_the_bundle() {
        let can_cross = &PRESETS[1];
        let mut ratios = RatioControls::default();
        let mut wave = GreenWave::default();
        let mut params = ElkParams::default();

        apply(can_cross, &mut ratios, &mut wave, &mut params);

        assert_eq!(ratios.cross_ratio, 0.0, "can-cross fords on natural drives, not the pull");
        assert_eq!(ratios.grass_regrow, 0.029);
        assert_eq!(wave.strength, 0.4);
        assert_eq!(params.grass_radius, 8.0, "can-cross widens forage perception");
        assert_eq!(params.freshness_weight, 4.0, "can-cross follows the green-up front");
        assert_eq!(params.sightline_weight, 3.0, "can-cross leapfrogs forward");
    }
}
