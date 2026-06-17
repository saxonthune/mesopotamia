# Perception Extraction + Field Overlays

## Motivation

Rung 2 of the observability ladder (doc02.03): agents steer by *fields* derived from terrain — the
grass gradient, the water-penalty surface, the aggregate desire — but nothing renders those fields,
only the terrain they come from. An overlay distinguishes a *cost* problem from a *perception*
problem: a flat grass gradient where you expected a pull means forage across a barrier is beyond
perception radius, and no crossing-cost slider fixes that. This rung is the one most likely to make
the elk-water bug self-evident.

## Do NOT

- Do NOT change `herd_move`'s behaviour. The grass-gradient extraction must be a pure refactor that
  `herd_move` calls — identical output, identical RNG order. Pin it with a test if unsure.
- Do NOT try to make the neighbour-dependent drives (sep/coh/social) samplable at empty cells — they
  need a per-tick neighbour snapshot and are agent-relative. Leave them inline in `herd_move`. Only
  the field-only drives (grass gradient, water penalty) are sampled on the grid.
- Do NOT let overlays affect the simulation; they are pure view state, off by default.

## Plan

### 1. Extract the grass-gradient as a pure function (`src/elk/movement.rs`)

The inline grass-gradient loop in `herd_move` (the `dy/dx` double loop summing `grid.forage(n)`)
becomes:

```rust
pub fn grass_gradient(cell: usize, grid: &Grid, params: &ElkParams) -> Vec2 { ... }
```

`herd_move` calls it where the inline loop was. Add metamorphic unit tests: gradient points toward
richer forage; zero when forage is flat; magnitude rises as a nearby cell's forage rises.

### 2. Water-penalty field sampler (`src/elk/movement.rs`)

`step_water_penalty` already exists and is pure. Add a thin helper that maps a cell to its penalty
(`step_water_penalty(grid.water(cell), grid.is_ford(cell), params.water_cost, params.ford_discount)`)
for grid sampling. The aggregate desire at an empty cell is out of scope (needs neighbours) — sample
only grass gradient and water penalty as fields.

### 3. Overlay toggle state + UI (`src/ui.rs`)

Mirror the existing `Graph`/`graphs_bar` toggle pattern. Add:

```rust
#[derive(Clone, Copy, PartialEq, Eq, Hash)] enum Overlay { GrassGradient, WaterPenalty }
```

with an `ALL` array and `label()`, and a `HashSet<Overlay>` on `UiState`. Add a toggle row (extend
`graphs_bar` or a sibling bar) so each overlay is switched on/off. Off by default.

### 4. Render the overlays (new `src/render` system or `src/overlay.rs`)

Draw over the world camera, below the egui camera. For `WaterPenalty`, a per-cell heatmap tint
(reuse the tile sprite layer or a gizmo rect per cell). For `GrassGradient`, a per-cell arrow
(Bevy gizmos line from cell center along `grass_gradient`). Sample on the visible grid; coarsen
(every Nth cell) if per-cell arrows are too dense. Gate the systems on the overlay being enabled.

## Files to Modify

- `src/elk/movement.rs` — extract `grass_gradient`, add the penalty sampler, unit tests.
- `src/elk/mod.rs` — export `grass_gradient` (and the penalty helper) for the overlay renderer.
- `src/ui.rs` — `Overlay` enum, `UiState` field, toggle row.
- `src/render.rs` (or a new `src/overlay.rs` registered as a plugin) — the gizmo/heatmap systems.
- `src/lib.rs` / `src/main.rs` + `src/bin/demo1.rs` — register the overlay plugin if new.

## Verification

```bash
just test-fast
cargo build
```

## Out of Scope

- The introspection panel (phase 1) and the death-map plot (phase 4 reuses this overlay infra).
- Sampling sep/coh/social at empty cells.

## Notes

- Chain `sim-observability`, phase 2 of 6. **Triage/build against phase 1's Surface**, not against a
  re-derivation of movement — phase 1 leaves `herd_move` behaviour-identical, which this phase must
  preserve too.
- `Grid` accessors used: `grid.forage(cell)`, `grid.water(cell)`, `grid.is_ford(cell)`,
  `grid.step(cell, dx, dy)`, `grid.col_row`/`width`. Confirm exact names against `src/grid.rs`.
- Rendering reference: `src/render.rs` world camera (the egui camera is order=1, full-window).
- This is the heaviest refactor in the chain — keep `grass_gradient` pure and `Grid`-only so it is
  both unit-testable and grid-samplable.

## Surface after this phase

- `pub fn grass_gradient(cell: usize, grid: &Grid, params: &ElkParams) -> Vec2` in
  `src/elk/movement.rs`, exported from `crate::elk`; `herd_move` calls it and is otherwise unchanged.
- A water-penalty-per-cell helper exported alongside it.
- An overlay rendering layer (plugin/systems) toggled via `UiState`'s `HashSet<Overlay>` with
  variants `GrassGradient`, `WaterPenalty`; a reusable per-cell gizmo/heatmap draw path that phase 4
  can extend for the death-map.
- Overlays are observational only; simulation behaviour and the balance metrics are unchanged.
