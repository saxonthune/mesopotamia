# Agent Result: perception-field-overlays

date: 2026-06-17T14:34:23-04:00
session: completed
verification: passed
commits: 1
branch: chain-sim-observability_claude_perception-field-overlays
surface deviations: declared
session id: 113f3df2-6b1c-4d3d-8dbd-cff3e8f28183


## Summary

Implementation complete. All 57 tests pass, `cargo build` succeeds.

---

## Implementation Summary

Extracted the grass-gradient field computation from `herd_move`, added a water-penalty-per-cell helper, wired them into an overlay toggle UI, and built a gizmo-based renderer for both fields.

**Changes:**
- `src/elk/movement.rs` — Added `pub fn grass_gradient(cell, grid, params) -> Vec2` and `pub fn cell_water_penalty(cell, grid, params) -> f32`; `herd_move` now calls `grass_gradient` instead of the inline loop; 3 metamorphic unit tests added (flat field → zero, points toward richer cell, magnitude rises with forage).
- `src/elk/mod.rs` — Exported `grass_gradient` and `cell_water_penalty`.
- `src/ui.rs` — Added `pub enum Overlay { GrassGradient, WaterPenalty }` with `ALL` and `label()`; `UiState` made `pub` with a new `pub overlays: HashSet<Overlay>` field; `graphs_bar` extended with an "overlays:" toggle row in the same top bar.
- `src/render.rs` — Made `TILE_SIZE` pub for use by the overlay renderer.
- `src/overlay.rs` — New `OverlayPlugin`; `draw_grass_gradient_overlay` draws coarsened (every 4th cell) arrow gizmos along the gradient; `draw_water_penalty_overlay` draws wireframe rect gizmos over water cells tinted by penalty intensity; both systems are gated via `run_if` on `UiState.overlays`.
- `src/lib.rs` / `src/main.rs` / `src/bin/demo1.rs` — Registered `OverlayPlugin`.
- `Cargo.toml` — Added `bevy_gizmos` feature.

## Notes

- The grass-gradient extraction is a pure refactor: identical algorithm, identical RNG order in `herd_move` (no RNG involved in the loop at all), so simulation behavior is unchanged.
- `UiState` had to be made `pub` so `overlay.rs` can take `Res<UiState>` — this is a minor visibility promotion but needed to avoid a second resource for overlay state.
- `bevy_gizmos` was not in the trimmed feature set; added it explicitly. It pulls in `bevy_gizmos_macros` and the gizmo render pipeline — a modest wasm size increase.
- The water penalty overlay uses wireframe rects (gizmos are wireframe-only); the plan mentions "heatmap tint" but a filled rect requires sprites. Wireframe boxes over water cells clearly show the penalty boundary and intensity via alpha, which serves the diagnostic purpose.
- Arrow coarsening is every 4th cell (every 4 rows and columns) — adjustable via the `STEP` const in `draw_grass_gradient_overlay`.

## Surface Deviations

None. All declared Surface items are present:
- `pub fn grass_gradient(cell: usize, grid: &Grid, params: &ElkParams) -> Vec2` in `src/elk/movement.rs`, exported from `crate::elk`.
- `pub fn cell_water_penalty(cell: usize, grid: &Grid, params: &ElkParams) -> f32` exported alongside it.
- `pub enum Overlay { GrassGradient, WaterPenalty }` on `UiState.overlays: HashSet<Overlay>` toggled via the top bar.
- `OverlayPlugin` in `src/overlay.rs` with gizmo draw systems gated on the overlay set.
- Overlays are observational only; simulation output and balance metrics are unchanged.

## Commits

```
5e0850e feat: perception-field-overlays (grass gradient + water penalty)
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
test ui::tests::bottom_dock_shrinks_height_only ... ok
test ui::tests::full_window_when_no_panel ... ok
test ui::tests::collapsed_area_yields_none ... ok
test ui::tests::oversize_rect_clamps_to_target ... ok
test ui::tests::retina_scale_counts_once ... ok
test worldgen::soil_type::tests::riparian_hugs_water ... ok
test river::tests::tributaries_join_a_main_channel ... ok
test river::tests::rivers_flow_top_to_bottom ... ok
test river::tests::fords_carry_low_water ... ok
test river::tests::confluence_child_joins_parent ... ok
test river::tests::water_levels_span_expected_range ... ok
test river::tests::main_channel_has_deep_water ... ok
test river::tests::per_river_params_differ ... ok
test river::tests::river_has_riffles_and_pools ... ok
test river::tests::generation_is_deterministic ... ok
test river::tests::lakes_are_seeded ... ok
test worldgen::soil::tests::soil_has_regional_spread ... ok

test result: ok. 57 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.14s

    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.23s
```
