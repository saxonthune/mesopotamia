# Agent Result: camping-fix

date: 2026-06-18T14:44:30-04:00
session: completed
verification: passed
commits: 5
branch: chain-camping-and-ui_claude_camping-fix
surface deviations: none
session id: aa62b0e7-c28c-4c10-a600-53ff8ee9fd33


## Summary

5 commits, all 85 tests green. Implementation complete.

---

## Implementation summary

All 8 plan steps implemented across 5 commits:

1. **Data model** (`components.rs`, `spawn.rs`, `metrics.rs`, `sim_harness.rs`): Added `pub intake_rate: f32` to `Elk`, initialized to `0.0` in all three construction sites; added `HabitatIntake { pub mean: f32 }` resource; added `intake_smoothing`, `giving_up`, `leave_boost` to `ElkParams` with defaults `0.05`, `0.6`, `1.5`.

2. **EWMA + habitat mean** (`metabolism.rs`): `graze` now computes `intake_this_tick` in all three branches (shrub, grass, none), applies the EWMA to `elk.intake_rate`, then takes a plain mean across all elk and writes it to `HabitatIntake`.

3. **Gate + drive change** (`movement.rs`, `mod.rs`, `sim_harness.rs`): Added `pub fn forage_gate(local_intake, habitat_mean, giving_up) -> f32` (linear ramp, bounded [0,1]); updated `combine_drives` to take `gate: f32`, scale grass and social by gate, and boost migration by `1 + leave_boost·(1−gate)`; updated `herd_move` to inject `Res<HabitatIntake>`, compute gate per elk, and pass it. Fixed all callers including the missed `FieldDecider::decide` in `sim_harness.rs`. Added 7 metamorphic tests for `forage_gate`.

4. **UI sliders** (`ui.rs`): Added "patch leaving" section at the bottom of `behaviour_tab` with sliders for `intake_smoothing`, `giving_up`, `leave_boost`.

5. **Docs** (`01-balance-metrics.md`): Added "Patch leaving" section with EWMA equation, habitat-mean definition, `forage_gate` formula, and application rule in `combine_drives`.

## Notes

One deviation from the plan during implementation: `sim_harness.rs::FieldDecider::decide` also called `combine_drives` and wasn't listed in the plan's "Files to Modify" section. It was caught by the first compile error and fixed by passing `gate = 1.0` (no patch-leaving suppression in the field-only decider, which is correct since it has no `intake_rate` context).

## Surface Deviations

None.

## Commits

```
6b837fb docs: add patch-leaving gate section to balance-metrics
f74a3a9 feat: add patch-leaving sliders to behaviour_tab
5f50bd4 feat: forage_gate, combine_drives gate param, herd_move patch-leaving logic
34858dd feat: update graze with EWMA intake_rate and habitat mean
e864738 feat: add intake_rate to Elk, HabitatIntake resource, patch-leaving params
```

## Build & Test Output (last 30 lines)

```
test elk::movement::tests::trailing_herd_crosses_for_the_far_bank ... ok
test field::tests::bare_field_with_no_floor_stays_bare ... ok
test field::tests::grass_spreads_into_a_bare_neighbour ... ok
test field::tests::normalize_flat_field_is_zero ... ok
test field::tests::normalize_stretches_to_unit_range ... ok
test field::tests::smooth_preserves_a_flat_field ... ok
test field::tests::zero_capacity_stays_empty ... ok
test grid::tests::reset_clears_all_per_cell_state ... ok
test grid::tests::total_grass_sums_every_cell ... ok
test metrics::tests::energy_extractor_returns_elk_energy ... ok
test metrics::tests::energy_extractor_zero ... ok
test ui::tests::collapsed_area_yields_none ... ok
test ui::tests::bottom_dock_shrinks_height_only ... ok
test ui::tests::full_window_when_no_panel ... ok
test ui::tests::retina_scale_counts_once ... ok
test ui::tests::oversize_rect_clamps_to_target ... ok
test worldgen::soil_type::tests::riparian_hugs_water ... ok
test river::tests::river_has_riffles_and_pools ... ok
test river::tests::rivers_flow_top_to_bottom ... ok
test river::tests::fords_carry_low_water ... ok
test river::tests::confluence_child_joins_parent ... ok
test river::tests::main_channel_has_deep_water ... ok
test river::tests::water_levels_span_expected_range ... ok
test river::tests::per_river_params_differ ... ok
test river::tests::tributaries_join_a_main_channel ... ok
test river::tests::generation_is_deterministic ... ok
test river::tests::lakes_are_seeded ... ok
test worldgen::soil::tests::soil_has_regional_spread ... ok

test result: ok. 85 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.11s
```
