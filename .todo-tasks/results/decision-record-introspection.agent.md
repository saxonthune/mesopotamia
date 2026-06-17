# Agent Result: decision-record-introspection

date: 2026-06-17T14:22:15-04:00
session: completed
verification: passed
commits: 1
branch: chain-sim-observability_claude_decision-record-introspection
surface deviations: none
session id: 5d8db3d1-484c-4248-b82a-761bd528ef6a


## Summary

Commit is recorded. The implementation is complete with 54/54 tests passing and a clean build.

---

## Implementation summary

All five plan steps were implemented:

1. **`Decomposable` trait + `impl` for `Drives`** (`movement.rs`): Added the trait and impl returning `[("sep", ...), ("coh", ...), ("grass", ...), ("social", ...), ("migration", ...)]`. Added two unit tests: label order matches `DRIVE_COLORS`, and `total()` equals vector sum of contributions.

2. **`Decision`/`StepEval` structs** (`movement.rs`): Added both structs with all public fields as specified, plus `Default` impl for `Decision` (needed for spawn-time initialization).

3. **`herd_move` updated** (`movement.rs`): Changed query to `Query<(&mut Elk, &mut LastDecision)>`, added `penalties` array to record per-step water penalties, moved `drives` into `decision_drives` after `desire` computation, tracked `chosen_k` via an `option_counter` in the pick loop, and wrote `Decision` to `last_decision.0` in both the hemmed-in branch and the normal path. Zero extra RNG calls; the single `rng.random_range` draw is in the same position.

4. **`LastDecision` component** (`components.rs`): Added with `use super::movement::Decision` and `#[derive(Component)] pub struct LastDecision(pub Decision)`. Spawn bundle in `spawn.rs` includes `LastDecision(Decision::default())`.

5. **UI** (`ui.rs`): Added `selected_elk: Option<Entity>` to `UiState`, updated `pick_herd` to use `Query<(Entity, &Elk)>` and set `state.selected_elk`. Added `Query<&LastDecision>` to `control_panel` params, threaded through to `herds_view` and `herd_details`. Added `elk_decision_panel` rendering the drive pie (via `Decomposable::contributions`) and the step-eval table with chosen marker and softmax probability.

## Notes

- The `Decomposable` trait and `Decision`/`StepEval` are defined in `movement.rs` as planned; `LastDecision` in `components.rs` imports `Decision` via `use super::movement::Decision` — a within-crate circular module reference that Rust handles without issue.
- `Decision` required a manual `Default` impl (Drives doesn't derive Default) to support spawn-time initialization.
- The hemmed-in `continue` branch now writes a `Decision { options: Vec::new(), chosen: None, ... }` before continuing, so every elk always has a valid `LastDecision` after the first tick it processes.

## Surface Deviations

None.

## Commits

```
88ad179 feat: Decision record, Decomposable, LastDecision, elk introspection panel
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
test grid::tests::total_grass_sums_every_cell ... ok
test grid::tests::reset_clears_all_per_cell_state ... ok
test ui::tests::bottom_dock_shrinks_height_only ... ok
test ui::tests::oversize_rect_clamps_to_target ... ok
test ui::tests::full_window_when_no_panel ... ok
test ui::tests::retina_scale_counts_once ... ok
test worldgen::soil_type::tests::riparian_hugs_water ... ok
test ui::tests::collapsed_area_yields_none ... ok
test river::tests::tributaries_join_a_main_channel ... ok
test river::tests::fords_carry_low_water ... ok
test river::tests::river_has_riffles_and_pools ... ok
test river::tests::main_channel_has_deep_water ... ok
test river::tests::per_river_params_differ ... ok
test river::tests::confluence_child_joins_parent ... ok
test river::tests::rivers_flow_top_to_bottom ... ok
test river::tests::water_levels_span_expected_range ... ok
test river::tests::generation_is_deterministic ... ok
test river::tests::lakes_are_seeded ... ok
test worldgen::soil::tests::soil_has_regional_spread ... ok

test result: ok. 54 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s

    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.20s
```
