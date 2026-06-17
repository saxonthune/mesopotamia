# Agent Result: spatial-event-log

date: 2026-06-17T15:02:16-04:00
session: completed
verification: passed
commits: 1
branch: chain-sim-observability_claude_spatial-event-log
surface deviations: declared
session id: fe2595d4-3471-4213-82b7-544b1cbe3392


## Summary

Implementation complete. Here's a summary:

**What was done:**

- **`src/events.rs`** (new): `EventKind` enum (extensible, `Starved` variant), `Event` struct (`tick`, `cell`, `kind`, `energy`, `chosen_step`), `EventLog` ring-buffer resource (cap 6000 to mirror `history.rs`), and `EventsPlugin`.
- **`src/lib.rs`**: Added `pub mod events;`.
- **`src/elk/mod.rs`**: Added `EventsPlugin` to `ElkSimPlugin::build`.
- **`src/elk/metabolism.rs`**: `metabolize` now takes `Res<Spawner>`, `ResMut<EventLog>`, and `Option<&LastDecision>` — pushes a `Starved` event (with tick from `spawner.elapsed`, cell, energy, and optionally the chosen step from `LastDecision`) at the despawn site.
- **`src/ui.rs`**: Added `DeathSites` overlay variant; `graphs_bar` now shows a floating "recent deaths" window listing the 50 most recent events (tick, cell, kind, energy, chosen step) when `DeathSites` is active.
- **`src/overlay.rs`**: Added `draw_death_sites_overlay` — draws an X gizmo at each death site with age-based alpha (old events are dimmer, recent ones brighter).
- **`src/bin/demo1.rs`**: Added `#[path = "../events.rs"] mod events;` (required because demo1 uses `#[path]` includes rather than linking the library).

## Notes

- The tick source uses `Spawner.elapsed` (already a u64-cast u32 per-tick counter from FixedUpdate) rather than adding a new `Tick` resource. This is semantically "ticks since start" and matches the sim's existing pattern.
- `demo1.rs` required a `#[path]` include for `events.rs` because it direct-includes sources into its own crate rather than linking `mesopotamia` as a library. This is a structural quirk of the binary.
- The "recent deaths" window is tied to the `DeathSites` overlay toggle — enabling the overlay shows both the world markers and the event list, which feels coherent. No separate toggle was added.

## Surface Deviations

None. All declared surface items are present:
- `src/events.rs` with `Event`, `EventKind` (`Starved` present, enum is extensible), and `EventLog` ring-buffer `Resource`, exported as `crate::events`.
- Mortality events emitted from `metabolize`, carrying cell + energy + optional chosen step from `LastDecision`.
- `DeathSites` overlay variant + recent-events UI list, both off by default (toggled via the overlay bar), observational only.

## Commits

```
4780767 feat: spatial-event-log — Starved ring-buffer, DeathSites overlay, recent-deaths panel
```

## Build & Test Output (last 30 lines)

```
test field::tests::grass_spreads_into_a_bare_neighbour ... ok
test field::tests::normalize_flat_field_is_zero ... ok
test field::tests::normalize_stretches_to_unit_range ... ok
test field::tests::smooth_preserves_a_flat_field ... ok
test field::tests::zero_capacity_stays_empty ... ok
test grid::tests::reset_clears_all_per_cell_state ... ok
test grid::tests::total_grass_sums_every_cell ... ok
test metrics::tests::energy_extractor_returns_elk_energy ... ok
test metrics::tests::energy_extractor_zero ... ok
test ui::tests::bottom_dock_shrinks_height_only ... ok
test ui::tests::oversize_rect_clamps_to_target ... ok
test ui::tests::retina_scale_counts_once ... ok
test ui::tests::collapsed_area_yields_none ... ok
test ui::tests::full_window_when_no_panel ... ok
test worldgen::soil_type::tests::riparian_hugs_water ... ok
test river::tests::river_has_riffles_and_pools ... ok
test river::tests::per_river_params_differ ... ok
test river::tests::tributaries_join_a_main_channel ... ok
test river::tests::confluence_child_joins_parent ... ok
test river::tests::fords_carry_low_water ... ok
test river::tests::main_channel_has_deep_water ... ok
test river::tests::rivers_flow_top_to_bottom ... ok
test river::tests::water_levels_span_expected_range ... ok
test river::tests::generation_is_deterministic ... ok
test river::tests::lakes_are_seeded ... ok
test worldgen::soil::tests::soil_has_regional_spread ... ok

test result: ok. 59 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.11s

    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.23s
```
