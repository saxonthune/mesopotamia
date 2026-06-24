# Agent Result: green-wave

date: 2026-06-24T00:03:06-04:00
session: completed
verification: passed
commits: 5
branch: milestone1_claude_green-wave
surface deviations: declared
session id: 894aa63f-c61f-43f8-8893-ead21cd2d7c1


## Summary

5 commits. All lib tests pass (204). Implementation is complete.

---

## Implementation Summary

Implemented the green wave feature across 4 steps:

**`src/field.rs`**: Added `green_wave(col, wavelength, tick, speed) -> f32` (cosine crest in [0,1]) and `wave_grow(field, cap, width, height, floor: &[f32], spread, senesce: &[f32]) -> Vec<f32>` (per-cell floor + senescence growth). Refactored `spread_grow` to delegate to `wave_grow` with constant floor and zero senesce vectors. Added 6 new tests covering crest advancement, periodicity, bounds, identity, senescence decay, and water-cell zeroing.

**`src/grid.rs`**: Added `GreenWave { strength, speed, wavelength }` resource (default: strength=0.5, speed=0.010, wavelength=85.0) and `SimTick` tick counter resource. Rewrote `growth` to build per-cell `floor` and `senesce` from the wave formula, calling `wave_grow`. At strength=0, formulas reduce exactly to `rate.intrinsic` floor and zero senesce — identity preserved. `grow_shrubs` and `forage()` are untouched.

**`src/sim_harness.rs` + `tests/herd_shape.rs`**: Added `centroid_col` harness helper. Added two `#[ignore]` diagnostics: `green_wave_crosses_on_worldgen` (real worldgen map, both wave-on and wave-off at zero pull) and `green_wave_crosses_on_plain` (open plain, same comparison). Both print centroid trajectories and a final advantage number.

**`.rhidoc/03-milestones/01-grazers/01-grazers.md`**: Added "The Green Wave" section describing standing grass crests, senescence in the trough, budget neutrality, and the strength=0 identity switch.

Also fixed a pre-existing broken state: `pub mod driftscape;` in `lib.rs` had no corresponding file, blocking `cargo test --lib`.

## Notes

- The `test-all` linker was killed (SIGTERM) — this is the documented machine limitation, not a code error. All 204 lib unit tests pass.
- CREST_GAIN and TROUGH_CUT are both 1.0, ensuring the wavelength-mean floor equals `rate.intrinsic` exactly (the "global budget neutral" constraint from the plan).
- SENESCE_MAX=0.05: at strength=0.5 and trough, grass decays at 2.5%/tick — gentle enough for a grazing herd but strong enough to clear stale ungrazed grass.
- The `driftscape.rs` stub creation was not in the plan but was required to compile; it's a minimal one-liner placeholder.
- The diagnostic tests will print useful numbers when run manually but assert nothing (by design — emergent and seed-noisy).

## Surface Deviations

None. All declared surface items are present with the exact specified signatures and behaviour:
- `field::green_wave(col, wavelength, tick, speed) -> f32` ✓
- `field::wave_grow(field, cap, width, height, floor: &[f32], spread, senesce: &[f32]) -> Vec<f32>` ✓
- `spread_grow` unchanged signature, now a thin wrapper ✓
- `grid::GreenWave { strength, speed, wavelength }` resource, `init_resource`d in `GridPlugin` ✓
- `strength == 0` reproduces pre-wave behaviour exactly ✓
- `growth` uses wave; `grow_shrubs` and shrub field unchanged ✓
- `forage()` and `grass_gradient` unchanged ✓
- `score.rs` untouched ✓

## Commits

```
4523f6a fix: remove unused Grid import in herd_shape diagnostic tests
4ae29fa doc: add green wave section to grazers milestone doc
4a495a7 feat: centroid_col harness helper; add #[ignore] green-wave diagnostic tests
346e5af feat: GreenWave resource + SimTick; rewrite growth to use wave_grow
1f1bb8e feat: green_wave + wave_grow in field.rs; fix missing driftscape stub
```

## Build & Test Output (last 30 lines)

```
test shrub_tile::tests::deeper_passes_add_more_recursion ... ok
test worldgen::rough::tests::strokes_are_spaced ... ok
test worldgen::soil_texture::tests::no_rough_means_no_speckle ... ok
test worldgen::soil_texture::tests::speckle_fills_apron_and_is_deterministic ... ok
test worldgen::soil_texture::tests::speckle_stays_within_reach_of_rough ... ok
test worldgen::soil_texture::tests::weight_peaks_beside_rough_and_fades_to_zero ... ok
test worldgen::soil_texture::tests::tint_one_always_has_a_horizontal_partner ... ok
test worldgen::soil_type::tests::riparian_hugs_water ... ok
test worldgen::vegetation::tests::coverage_threshold_ignores_ineligible_cells ... ok
test worldgen::vegetation::tests::coverage_threshold_keeps_the_target_fraction ... ok
test worldgen::vegetation::tests::coverage_threshold_no_eligible_keeps_nothing ... ok
test worldgen::vegetation::tests::no_rough_blocks_nothing ... ok
test worldgen::vegetation::tests::wet_tiles_bear_no_shrubs ... ok
test worldgen::vegetation::tests::rough_border_is_cleared ... ok
test river::tests::per_river_params_differ ... ok
test river::tests::tributaries_join_a_main_channel ... ok
test river::tests::rivers_flow_top_to_bottom ... ok
test river::lake::tests::big_lakes_are_interior_and_clear_of_borders ... ok
test river::tests::main_channel_has_deep_water ... ok
test river::tests::water_levels_span_expected_range ... ok
test river::tests::river_has_riffles_and_pools ... ok
test river::tests::fords_carry_low_water ... ok
test river::tests::confluence_child_joins_parent ... ok
test worldgen::soil::tests::grain_roughens_neighbours ... ok
test worldgen::soil::tests::soil_has_regional_spread ... ok
test shrub_tile::tests::void_fill_never_removes_ink ... ok
test river::tests::lakes_are_seeded ... ok
test river::tests::generation_is_deterministic ... ok

test result: ok. 204 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.35s
```
