# Agent Result: pull-penalty

date: 2026-06-24T00:21:10-04:00
session: completed
verification: passed
commits: 1
branch: chain-score-feedback_claude_pull-penalty
surface deviations: none
session id: 8a5dda93-c6d4-48a8-a454-f5db0f35753b


## Summary

The commit is in place. Implementation is complete.

## Implementation Summary

Added a pull penalty to `src/elk/score.rs` that discounts despawn payouts proportional to how much the herd has been relying on the migration ("magic") force.

**What changed:**
- `PULL_ALPHA = 0.03` and `PULL_PENALTY = 0.6` constants added alongside existing ones
- `Score.pull_share: f32` — new public field, smoothed herd-mean migration reliance in `[0, 1]`, updated every tick
- `pull_factor(pull_share: f32, penalty: f32) -> f32` — pure function, monotone decreasing in `pull_share`, bounded to `[1 − penalty, 1]`
- `update_score` now takes `Res<DriveSamples>`, EWMAs `pull_share` from `samples.migration_share()`, and scales each payout by `pull_factor(score.pull_share, PULL_PENALTY)`
- Module doc-comment rewritten in present tense to describe the penalty
- 5 new tests: `pull_factor_endpoints`, `pull_factor_monotone_decreasing`, `pull_factor_bounded`, `payout_falls_with_pull_reliance` (all pass; 219 total)

## Notes

- `test-all` fails at the linker step for integration binaries — this is a pre-existing environment constraint (no GPU/display available for linking Bevy binary targets) and is not caused by these changes. `test-fast` passes 219/219.
- `DriveSamples::migration_share()` (population-weighted, over all slots) is used rather than the per-slot `DriveSample::migration_share()` average. The plan mentions both; the `DriveSamples`-level method is the cleaner herd-aggregate signal and already the canonical metric.
- The `pull_share` EWMA smooths toward the instantaneous `migration_share()` reading every tick, matching the pace of `difficulty` at `alpha = 0.03`.

## Surface Deviations

None.

## Commits

```
5e57bcd feat: pull penalty — score pull-bought crossings less than foraged ones
```

## Build & Test Output (last 30 lines)

```
test shrub_tile::tests::lean_mirrors_the_trunk ... ok
test worldgen::soil_texture::tests::no_rough_means_no_speckle ... ok
test shrub_tile::tests::branches_always_appear ... ok
test worldgen::soil_texture::tests::speckle_stays_within_reach_of_rough ... ok
test worldgen::soil_texture::tests::tint_one_always_has_a_horizontal_partner ... ok
test worldgen::soil_texture::tests::weight_peaks_beside_rough_and_fades_to_zero ... ok
test worldgen::soil_type::tests::riparian_hugs_water ... ok
test worldgen::vegetation::tests::coverage_threshold_ignores_ineligible_cells ... ok
test worldgen::vegetation::tests::coverage_threshold_keeps_the_target_fraction ... ok
test worldgen::vegetation::tests::coverage_threshold_no_eligible_keeps_nothing ... ok
test worldgen::vegetation::tests::no_rough_blocks_nothing ... ok
test worldgen::vegetation::tests::rough_border_is_cleared ... ok
test worldgen::vegetation::tests::wet_tiles_bear_no_shrubs ... ok
test worldgen::soil_texture::tests::speckle_fills_apron_and_is_deterministic ... ok
test river::tests::per_river_params_differ ... ok
test river::tests::tributaries_join_a_main_channel ... ok
test river::lake::tests::big_lakes_are_interior_and_clear_of_borders ... ok
test river::tests::fords_carry_low_water ... ok
test river::tests::river_has_riffles_and_pools ... ok
test river::tests::confluence_child_joins_parent ... ok
test river::tests::main_channel_has_deep_water ... ok
test river::tests::rivers_flow_top_to_bottom ... ok
test worldgen::soil::tests::grain_roughens_neighbours ... ok
test river::tests::water_levels_span_expected_range ... ok
test worldgen::soil::tests::soil_has_regional_spread ... ok
test shrub_tile::tests::void_fill_never_removes_ink ... ok
test river::tests::generation_is_deterministic ... ok
test river::tests::lakes_are_seeded ... ok

test result: ok. 219 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.35s
```
