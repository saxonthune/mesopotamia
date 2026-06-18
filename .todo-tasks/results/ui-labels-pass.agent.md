# Agent Result: ui-labels-pass

date: 2026-06-18T15:05:27-04:00
session: completed
verification: passed
commits: 1
branch: chain-camping-and-ui_claude_ui-labels-pass
surface deviations: none
session id: 97f4a71f-7db3-4ab5-abf1-9248ee07ebee


## Summary

Implementation complete. All 89 tests pass and the commit is in place.

---

The changes made to `src/ui.rs`:

1. **Drive labels restored** — added `DRIVE_LABELS: [&str; 5]` with short display names (Separation, Cohesion, Forage, Foraging cue, Migration). Changed `PieSpec.slices` from a 3-tuple to a 4-tuple `(label, explain, value, color)`. `render_pie` now renders a vertical legend with a coloured swatch + visible text label per drive; full explanation and weight appear on hover as the deeper disclosure tier.

2. **Abbreviations expanded** — `"mig share"` → `"migration share"` (plot series), `"forage/elk"` → `"forage per elk"`, `"regrowth÷drain"` → `"regrowth ÷ drain"`, `"· gone"` → `"· no survivors"`, `"alive {} · peak {} · slot {}"` → `"alive: {}  peak: {}  cohort slot: {}"`, `"slot:"` → `"cohort slot:"`, `"gone"` in status → `"no survivors"`.

3. **Panel labels added** — `"drive composition:"` heading before the herd drive pie, `"drive breakdown:"` heading before the elk decision pie, `"step options (hover for scores):"` label before the step-arrow row.

4. **Patch-leaving / ratio sliders** — already had clear full-word labels from prior phases; no changes needed.

## Notes

- The `render_pie` legend changed from a horizontal row of bare swatches to a vertical list of `[swatch] Label` rows. This uses slightly more vertical space in the panel, which is appropriate given the plan's mandate for visible labels over colour-only communication.
- The old comment in `render_pie` ("product rule: no labels, tooltips when feasible") and the `DRIVE_EXPLAIN` docstring have been updated to reflect the corrected rule (visible labels first, hover as deeper tier).
- No model, metric, slider-semantics, or parameter-value changes were made.

## Surface Deviations

None.

## Commits

```
49734df feat: ui-labels-pass — expand abbreviations, restore drive labels, label panels
```

## Build & Test Output (last 30 lines)

```
test elk::ratios::tests::regrow_ratio_round_trips ... ok
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
test ui::tests::oversize_rect_clamps_to_target ... ok
test worldgen::soil_type::tests::riparian_hugs_water ... ok
test ui::tests::bottom_dock_shrinks_height_only ... ok
test ui::tests::retina_scale_counts_once ... ok
test ui::tests::full_window_when_no_panel ... ok
test river::tests::river_has_riffles_and_pools ... ok
test river::tests::main_channel_has_deep_water ... ok
test river::tests::water_levels_span_expected_range ... ok
test river::tests::per_river_params_differ ... ok
test river::tests::tributaries_join_a_main_channel ... ok
test river::tests::rivers_flow_top_to_bottom ... ok
test river::tests::confluence_child_joins_parent ... ok
test river::tests::fords_carry_low_water ... ok
test river::tests::lakes_are_seeded ... ok
test river::tests::generation_is_deterministic ... ok
test worldgen::soil::tests::soil_has_regional_spread ... ok

test result: ok. 89 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.12s
```
