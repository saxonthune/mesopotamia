# Declarative UI panels (Item list + progressive disclosure)

## Motivation

The demo's control/readout panels need to be composed declaratively — the consumer lists what
goes in a panel, in order, and the migration-vs-natural readouts (graphs/pie, a later task) drop
in as items. Today a `Panel`'s body is one opaque closure (`src/ui.rs:47-88`), so it can't be
composed, reordered, or progressively disclosed. This task evolves `Panel` from "one body
closure" to "an ordered list of items", adds progressive disclosure, and keeps the existing tabs
working via an escape hatch. It is the refactor half of the UI work; the graph/pie widgets are a
separate follow-on (`ui-graphs`).

Follows doc02.01 (small, composable; the renderer is one concept). This is phase 3a.

## Do NOT

- **Do NOT** do a big-bang rewrite. Keep `Item::Custom(closure)` so `grass_tab`,
  `behaviour_tab`, `view_tab` keep working unchanged during and after migration.
- **Do NOT** add graph widgets, the pie, `egui_plot`, or any sampling here — those are
  `ui-graphs` (phase 3b). `Item::Plot`/`Item::Pie` variants are NOT added in this task.
- **Do NOT** change `panel_flow`'s flex-wrap layout — this task changes what fills a panel, not
  how panels are arranged.
- **Do NOT** regress `world_viewport` / `set_camera_viewport` or their unit tests.

## Plan

### 1. The Item enum + renderer (`src/ui.rs`)

```rust
enum Item<'a> {
    Slider { value: &'a mut f32, range: std::ops::RangeInclusive<f32>, label: &'a str },
    Label(String),
    Separator,
    Section { title: &'a str, items: Vec<Item<'a>>, default_open: bool },
    Custom(Box<dyn FnOnce(&mut egui::Ui) + 'a>),
}
```

A `Panel` holds `Vec<Item>` instead of a single body closure. A `render_items(ui, items)` walks
the list: `Slider` → the existing `slider` helper; `Label`/`Separator` → the obvious egui calls;
`Section` → `egui::CollapsingHeader::new(title).default_open(default_open)` wrapping a recursive
`render_items` (this is the progressive disclosure); `Custom` → call the closure.

### 2. Migrate Panel (`src/ui.rs`)

Change `Panel`'s `body` field to `items: Vec<Item>`; `panel_flow` calls `render_items` in place of
`(p.body)(ui)`. Keep `Panel::new`/`width` ergonomics (e.g. `Panel::new(title, items)` or a small
builder). Update the `control_panel` Sliders tab: wrap the existing `*_tab` functions as
`Item::Custom` so they render exactly as before — proving the escape hatch and keeping behaviour
identical.

### 3. Demonstrate the declarative form on one panel

Convert **one** real panel (the Grass panel is smallest) from a `Custom` closure to explicit
`Item`s (Sliders + a `Section` for an "advanced" group) so the new API is exercised end to end
and progressive disclosure is visible. Leave `behaviour_tab`/`view_tab` as `Custom` for now.

### 4. Tests

The UI is immediate-mode and hard to unit-test; keep the existing `world_viewport` tests green.
No new rendering tests required — verify by building and the smoke check.

## Files to Modify

- `src/ui.rs` — `Item` enum, `render_items`, `Panel` → `Vec<Item>`, migrate the Grass panel,
  wrap other tabs as `Custom`.

## Verification

```sh
cargo build --bin mesopotamia
cargo build --bin demo1
cargo test
bash smoke.sh --no-run
```

## Out of Scope

- `egui_plot`, line graphs, pie chart, sampling/history (the `ui-graphs` task).
- Migrating every tab to the item list — only the Grass panel, as a demonstration.

## Notes

- Independent of `drive-sampling-resource` — touches only `ui.rs`, so it runs in parallel with
  that task after `drive-decomposition-metric` merges.
- `ui-graphs` (phase 3b) extends `Item` with `Plot`/`Pie` and consumes this renderer, so keep
  `Item` and `render_items` easy to extend (non-exhaustive match arms / a clear add point).

## Surface after this phase

- `src/ui.rs`: `enum Item<'a>` with `Slider`, `Label`, `Separator`, `Section { default_open }`,
  `Custom`; a `render_items(&mut egui::Ui, Vec<Item>)` walker; `Panel` holds `Vec<Item>` and is
  laid out by the unchanged `panel_flow`. `Section` renders as a `CollapsingHeader` (progressive
  disclosure).
- The Sliders tab renders identically to before (tabs wrapped as `Custom`); the Grass panel is
  expressed as explicit `Item`s including one `Section`.
- Negative space: no `Plot`/`Pie` variants yet, no graph crate, no sampling — added by `ui-graphs`.
