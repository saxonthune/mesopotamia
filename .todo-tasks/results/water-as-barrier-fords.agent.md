# Agent Result: water-as-barrier-fords

date: 2026-06-14T23:58:05-04:00
session: completed
verification: passed
commits: 1
branch: chain-rivers-and-fords_claude_water-as-barrier-fords
surface deviations: none
session id: 43d8a4bd-fc64-42cf-9cdc-f183f2cb0c80


## Summary

All done. Here's a summary:

---

## Implementation Summary

Implemented all five steps from the plan across `src/elk.rs` and `src/ui.rs`:

1. **`step_water_penalty` pure helper** (`elk.rs`) — takes `(water, is_ford, water_cost, ford_discount)`. On a ford: `water * water_cost * ford_discount` (near-free). Off a ford: `water * water_cost` (today's behaviour, numerically identical).

2. **Wired into `herd_move` scoring** — replaced the inline `- grid.water(next) * params.water_cost` expression with a call to `step_water_penalty(...)` passing `grid.is_ford(next)` and `params.ford_discount`.

3. **Swim energy drain** — after the step-pick loop, if the entered cell has `water > 0` and is not a ford, subtract `swim_drain * water` from `elk.energy` (clamped at 0).

4. **`ElkParams` gains `ford_discount: 0.1` and `swim_drain: 0.01`** in the struct and `Default` impl.

5. **Three UI sliders** added to `behaviour_tab`: `water_cost` (0..=4), `ford_discount` (0..=1), `swim_drain` (0..=0.05). The existing `water_cost` slider's upper bound was adjusted from 5 to 4 to match the plan.

**Tests**: 4 new unit tests for `step_water_penalty` (example, exact match, monotonicity, deep > shallow) + all 27 tests pass. Clippy clean.

## Notes

- The plan's `water_cost` slider range is `0..=4`; the existing code had `0..=5`. I used the plan's declared range. This is a minor UI-only difference.
- The swim drain applies for any `water > 0` cell (not just above a threshold), since the drain already scales with water depth — shallow tributaries (water ~0.1) contribute only 0.001 drain per tick, negligible.
- `step_water_penalty` is `pub` so tests and any future metric extraction can call it directly.

## Surface Deviations

None.

## Commits

```
92a8bde feat: water-as-barrier fords — step penalty, swim drain, UI knobs
```

## Build & Test Output (last 30 lines)

```
test elk::tests::penalty_monotone_in_water ... ok
test elk::tests::more_forage_across_never_lowers_cross_desire ... ok
test elk::tests::trailing_herd_crosses_for_the_far_bank ... ok
test field::tests::bare_field_with_no_floor_stays_bare ... ok
test field::tests::grass_spreads_into_a_bare_neighbour ... ok
test field::tests::normalize_flat_field_is_zero ... ok
test field::tests::normalize_stretches_to_unit_range ... ok
test field::tests::smooth_preserves_a_flat_field ... ok
test field::tests::zero_capacity_stays_empty ... ok
test river::tests::ford_indices_empty_centerline ... ok
test river::tests::ford_indices_returns_every_nth_cell ... ok
test river::tests::ford_indices_spacing_larger_than_len ... ok
test ui::tests::collapsed_area_yields_none ... ok
test ui::tests::bottom_dock_shrinks_height_only ... ok
test ui::tests::full_window_when_no_panel ... ok
test ui::tests::oversize_rect_clamps_to_target ... ok
test ui::tests::retina_scale_counts_once ... ok
test river::tests::fords_carry_low_water ... ok
test river::tests::main_channel_has_deep_water ... ok
test river::tests::water_levels_span_expected_range ... ok
test river::tests::generation_is_deterministic ... ok

test result: ok. 27 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.13s
warning: the following packages contain code that will be rejected by a future version of Rust: block v0.1.6
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 1`
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.23s
warning: the following packages contain code that will be rejected by a future version of Rust: block v0.1.6
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 1`
```
