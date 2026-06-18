# Agent Result: ratio-sliders

date: 2026-06-18T14:57:56-04:00
session: completed
verification: passed
commits: 2
branch: chain-camping-and-ui_claude_ratio-sliders
surface deviations: none
session id: 3200ae66-f430-4b13-864d-cc3e449180d4


## Summary

89 tests pass, 2 commits made. Implementation is complete.

---

## Implementation summary

Created `src/elk/ratios.rs` with `RatioControls { bite_ratio, regrow_ratio, cross_ratio }` (a Bevy `Resource`), three pure derivation functions, and the `apply_ratios` system. Wired it into `ElkSimPlugin` (resource init + Update system). Rewired three UI sliders in `src/ui.rs` to bind directly to `RatioControls` fields via split field borrows. Updated doc04.02 to name and define all three ratios.

## Notes

- `apply_ratios` runs in `Update` (unconditionally, not inside `run_if(Sim::Running)`) so the break-even label in the UI always reflects current ratios even while paused.
- Guards against division by zero when `bite == 0.0` or derived `graze_yield == 0.0`, leaving the existing value in place rather than writing NaN.
- The break-even grazing label in `behaviour_tab` still reads `p.graze_yield` — this is the value written by `apply_ratios` each frame, so it stays live and correct.
- `behaviour_tab` signature changed from `(ui, p)` to `(ui, p, bite_ratio, cross_ratio)` — the split-borrow pattern allows `regrow_ratio` to live in the `grass_items` vec while `bite_ratio`/`cross_ratio` are captured by the behaviour closure simultaneously.
- Slider ranges: `bite_ratio` 0.1–20.0, `regrow_ratio` 0.0–1.0, `cross_ratio` 0.0–2.0.

## Surface Deviations

None.

## Commits

```
e8eebe8 docs: name bite_ratio, regrow_ratio, cross_ratio in parameter discipline
88e77b8 feat: ratio sliders for bite, regrowth, and migration
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
test ui::tests::bottom_dock_shrinks_height_only ... ok
test ui::tests::full_window_when_no_panel ... ok
test ui::tests::oversize_rect_clamps_to_target ... ok
test ui::tests::retina_scale_counts_once ... ok
test worldgen::soil_type::tests::riparian_hugs_water ... ok
test river::tests::tributaries_join_a_main_channel ... ok
test river::tests::rivers_flow_top_to_bottom ... ok
test river::tests::per_river_params_differ ... ok
test river::tests::confluence_child_joins_parent ... ok
test river::tests::main_channel_has_deep_water ... ok
test river::tests::fords_carry_low_water ... ok
test river::tests::river_has_riffles_and_pools ... ok
test river::tests::water_levels_span_expected_range ... ok
test river::tests::generation_is_deterministic ... ok
test worldgen::soil::tests::soil_has_regional_spread ... ok
test river::tests::lakes_are_seeded ... ok

test result: ok. 89 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s
```
