# Agent Result: value-fn-and-graze-gate

date: 2026-06-19T14:49:14-04:00
session: completed
verification: passed
commits: 1
branch: chain-elk-decision-palette_claude_value-fn-and-graze-gate
surface deviations: none
session id: 1bcf5fa0-df7b-431a-bed1-b9238d816ffe


## Summary

All 108 tests green, one commit made.

## Implementation summary

Implemented the plan across four files:

**`src/elk/movement.rs`** — Added `graze_value(here_forage, energy, dwell) -> f32` and `stand_value() -> f32` pure functions. Refactored `herd_move` to use `[_; 6]` arrays (indices 0–3: Step, 4: Stand, 5: Graze), extended softmax and weight computation over all six, and replaced the pick loop with a labeled `'pick` block that dispatches `Act::Stand`/`Act::Graze` (leave `elk.cell` unchanged) vs `Act::Step(dx, dy)` (move the elk). Added 5 unit tests: satiation indifference, monotonicity in forage, monotonicity in hunger, graze-beats-stand when hungry on forage, and graze-never-worse-than-stand.

**`src/elk/metabolism.rs`** — Imported `Act`; added `&LastDecision` to the `graze` query; eating (shrub/grass) now occurs only when `chosen_act == Act::Graze`. `elk.grazing = false` and `intake_this_tick = 0.0` for movers/standers. The `intake_rate` EWMA update and `HabitatIntake` bookkeeping run for every elk regardless.

**`src/elk/components.rs`** — Added `pub dwell: f32` to `ElkParams` with doc comment. Set `dwell: 1.5` in defaults. Retuned `bite: 0.5` → `bite: 0.12` (multi-tick dwell instead of one-tick strip). Updated comments.

**`src/elk/mod.rs`** — Exported `graze_value` and `stand_value`.

**`src/sim_harness.rs`** — Imported `graze_value`/`stand_value`; `FieldDecider::decide` now appends Stand and Graze candidates and includes them in the deterministic best-pick.

## Notes

- The hemmed-in branch in `herd_move` (`if !max.is_finite()`) is now unreachable in normal conditions since Stand always scores 0.0 and Graze ≥ 0.0. Kept it per spec for the empty-options UI case.
- With `bite = 0.12` and `graze_yield = 0.035`, a single graze tick yields ≈ 0.0042 energy vs drain of 0.004 — barely positive per tick, so the elk must dwell for many ticks to meaningfully recover. This is intentional (multi-tick pause) but means energy recovery is slow; `dwell` is the primary lever if grazing is too rare.
- Gate-feedback risk flagged in the plan (mover → 0-intake → gate closes → more migration) is a systemic check for live observation / `test-all`, not caught by unit tests.
- The `FieldDecider` uses a deterministic best-pick (no softmax) so it may pick Graze more aggressively than the probabilistic herd_move on rich forage cells.

## Surface Deviations

None.

## Commits

```
dcbd2b1 feat: unified value fn + graze gate (phase 2 elk-decision-palette)
```

## Build & Test Output (last 30 lines)

```
test river::lake::tests::basin_offset_kernels_are_asymmetric ... ok
test river::lake::tests::basin_single_kernel_is_round ... ok
test river::lake::tests::lake_depth_below_iso_is_zero ... ok
test river::lake::tests::lake_depth_rises_above_iso_and_clamps ... ok
test river::lake::tests::lake_positions_are_spaced ... ok
test ui::tests::full_window_when_no_panel ... ok
test ui::tests::retina_scale_counts_once ... ok
test ui::tests::oversize_rect_clamps_to_target ... ok
test ui::tests::bottom_dock_shrinks_height_only ... ok
test ui::tests::collapsed_area_yields_none ... ok
test ui::tests::max_delta_is_monotone_non_increasing_in_speed ... ok
test ui::tests::fast_speeds_bound_scaled_advance_to_budget ... ok
test worldgen::rough::tests::intensity_rises_with_dryness ... ok
test worldgen::rough::tests::zero_below_threshold ... ok
test worldgen::rough::tests::zero_in_water ... ok
test worldgen::soil_type::tests::riparian_hugs_water ... ok
test ui::tests::slow_speeds_keep_engine_default ... ok
test river::tests::tributaries_join_a_main_channel ... ok
test river::tests::fords_carry_low_water ... ok
test river::tests::confluence_child_joins_parent ... ok
test river::tests::water_levels_span_expected_range ... ok
test river::tests::main_channel_has_deep_water ... ok
test river::tests::per_river_params_differ ... ok
test river::tests::river_has_riffles_and_pools ... ok
test river::tests::rivers_flow_top_to_bottom ... ok
test river::tests::lakes_are_seeded ... ok
test river::tests::generation_is_deterministic ... ok
test worldgen::soil::tests::soil_has_regional_spread ... ok

test result: ok. 108 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.11s
```
