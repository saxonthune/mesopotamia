# UI graphs (line graph + pie) and the force-share readouts

## Motivation

The demo surfaces, for a selected herd, the migration-vs-natural force split (a pie of the five
drive contributions) and how population / energy / migration-share evolve over time (line graphs)
— doc03.01.01, doc02.02. This task adds the two graph widgets to the declarative panel system
(`Item` list from `ui-item-list`) and a time-series history buffer that samples `DriveSamples`
(from `drive-sampling-resource`), then wires the readouts into the Herds detail pane.

Follows doc02.01 (each widget is one concept; the history sampler is one concept). This is
phase 3b. Read doc02.02 for what the pie and the share graph mean.

## Do NOT

- **Do NOT** recompute drive shares — read `DriveSamples`/`DriveSample::migration_share` (from
  the `drive-sampling-resource` Surface). The history buffer samples that resource; it does not
  touch `movement.rs`.
- **Do NOT** re-do the `Panel`/`Item` refactor — extend the existing `Item` enum and
  `render_items` from the `ui-item-list` Surface.
- **Do NOT** add a heavyweight charting dependency — `egui_plot` for lines, a hand-rolled
  `Painter` pie (~30 lines). Match `egui_plot` to the egui version (egui 0.33 → `egui_plot = "0.33"`).
- **Do NOT** regress `world_viewport` / `set_camera_viewport` tests or the `ui-item-list` Grass
  panel demonstration.

## Plan

### 1. History resource (`src/history.rs`, re-exported by lib)

A ring buffer sampling per-tick aggregates for the graphs:

```rust
#[derive(Resource, Default)]
pub struct History {
    pub population: VecDeque<f32>,
    pub avg_energy: VecDeque<f32>,
    pub migration_share: Vec<VecDeque<f32>>, // per slot, from DriveSamples
}
```

A `sample_history` system (on `Update` or `FixedUpdate`) pushes the current population (Elk
count), average energy (from `Herds`), and per-slot `DriveSamples[slot].migration_share()`,
capping each buffer to a fixed window (e.g. 600 samples). Register it in `UiPlugin` (or a tiny
`HistoryPlugin`). It reads existing resources only — no sim behaviour change.

### 2. Extend Item with graph variants (`src/ui.rs`)

Add to the `Item` enum from `ui-item-list`:

```rust
Plot(PlotSpec<'a>),  // labelled series of [f64; 2] read from History
Pie(PieSpec<'a>),    // Vec<(label, value, egui::Color32)>
```

`render_items` gains arms for them.

### 3. Line graph via egui_plot (`Cargo.toml`, `src/ui.rs`)

Add `egui_plot = "0.33"`. `PlotSpec` carries one or more named series; the render arm builds an
`egui_plot::Plot` with `Line`s from the points. Use `bevy_egui`'s re-exported `egui` types to
avoid a version mismatch.

### 4. Pie chart painter (`src/ui.rs`)

`render_pie(ui, slices: &[(String, f32, egui::Color32)])`: allocate a square rect, get the
`Painter`, and draw filled wedges (`Shape::convex_polygon` or a triangle fan per slice) with each
slice's angular span proportional to its value; a small legend below. ~30 lines, no crate.

### 5. Wire the demo readouts (`src/ui.rs` `herd_details`)

In the selected-herd detail pane, add:
- a **Pie** of the five drive magnitudes for the herd's slot from `DriveSamples` (natural four +
  migration), so the magic-force share is visible at a glance (doc02.02 migration_share);
- **Plots** of population and that slot's migration_share over time from `History`.

## Files to Modify

- `Cargo.toml` — add `egui_plot = "0.33"`.
- `src/history.rs` — NEW. `History` resource + `sample_history` system.
- `src/lib.rs` — `pub mod history;`.
- `src/ui.rs` — `Item::Plot`/`Item::Pie` + render arms, `egui_plot` line widget, pie painter,
  wire into `herd_details`; register the history sampler.

## Verification

```sh
cargo build --bin mesopotamia
cargo build --bin demo1
cargo test
bash smoke.sh --no-run
```

## Out of Scope

- The sweep / balancing harness (separate task).
- Migrating remaining tabs to the item list beyond what the readouts need.

## Notes

- Depends on TWO Surfaces: `ui-item-list` (`Item` enum + `render_items` + `Panel` as `Vec<Item>`)
  and `drive-sampling-resource` (`DriveSamples`, `DriveSample::migration_share`). Triage against
  both; run after both merge. It is parallel with `balancing-param-sweep` (disjoint files: `ui.rs`
  + `history.rs` + `Cargo.toml` vs `sim_harness.rs` + `bin/`).
- Confirm `egui_plot`'s egui dependency resolves to the same 0.33.x `bevy_egui` uses; if a 0.33
  `egui_plot` is unavailable, pin the closest matching minor and note it.

## Surface after this phase

- `mesopotamia::history::History` resource + `sample_history` system populating population,
  avg energy, and per-slot migration_share ring buffers.
- `src/ui.rs`: `Item::Plot`/`Item::Pie` variants, an `egui_plot` line-graph render arm, a
  `Painter` pie helper, and `herd_details` showing the drive-split pie + population/share plots
  for the selected herd.
- `egui_plot` dependency in `Cargo.toml`.
