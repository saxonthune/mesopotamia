# Split elk.rs into an elk/ module directory

## Motivation

`elk.rs` (~600 lines) does six jobs: plugin/components, spawning + lifecycle, color,
the boids movement algorithm, the lifecycle systems, and the `cross_desire` spec. Big
mixed files are hard for both author and headless agents to navigate. `doc03.01.02`
sanctions the split ("a module grows into a `foo.rs` + `foo/` directory pair only when it
holds enough to split") — `elk.rs` now clearly holds enough.

## Sequencing — runs `--after` `sim-render-plugin-split`

Depends on `sim-render-plugin-split` having merged: that task removes the render systems
from `elk.rs` first, so this split organises only the simulation seams. It also depends on
`macro-state-test-harness` (the regression gate). Launch only after both merge.

## Do NOT

- **Do NOT change behaviour, signatures, or tuning.** This is a pure file reorganisation:
  same items, same logic, moved into submodules with the right `pub(crate)` visibility.
- **Do NOT break `tests/macro_sim.rs` or the `cross_desire` unit tests.** Move the
  `#[cfg(test)]` module with `cross_desire` to wherever `cross_desire` lands; all tests pass
  unchanged.
- **Do NOT re-introduce render systems here** — they left in the previous task.
- **Do NOT touch movement/grazing logic** while moving it. Relocation only.

## Plan

### 1. Create the `elk/` directory along the seams

Convert `src/elk.rs` into `src/elk.rs` (kept as the module root re-exporting submodules) +
`src/elk/` with submodules split along the existing seams:

- `elk/components.rs` — `Elk`, `Cohort`, `Herds`, `Packs`, `Spawner`, `ElkParams` and their
  `impl`s.
- `elk/spawn.rs` — `spawn_pack`, `spawn_waves`, `cull`, `tally_herds`.
- `elk/movement.rs` — `herd_move`, `norm`, separation/cohesion helpers, `cross_desire` and
  its `#[cfg(test)]` tests.
- `elk/metabolism.rs` — `graze`, `digest`, `metabolize`, `migrate_pressure`.
- `elk/color.rs` — `elk_color` (`pub(crate)`).

`elk.rs` (root) keeps `ElkSimPlugin` and `pub use` the items other modules import
(`Elk`, `ElkParams`, `Packs`, `Herds`, `PACK_COUNT`, `elk_color`, `cross_desire`, …) so
external paths (`mesopotamia::elk::Elk`, `render.rs`'s `elk_color` import) keep working.

### 2. Fix visibility

Items used across submodules become `pub(crate)`; items only used within one submodule stay
private. Consts (`TARGET_POPULATION`, `EDGE_COL`, etc.) move next to their users.

### 3. Reconcile `doc03.01.02` in place

Update the crate-layout section to show the `elk.rs` + `elk/` pair and what each submodule
owns. Present-tense declarative prose; rewrite in place.

## Files to Modify

- `src/elk.rs` — becomes the module root (`ElkSimPlugin` + `pub use` re-exports).
- `src/elk/components.rs`, `src/elk/spawn.rs`, `src/elk/movement.rs`,
  `src/elk/metabolism.rs`, `src/elk/color.rs` — NEW submodules.
- `.carta/03-milestones/01-grazers/02-structure.md` — show the `elk/` layout.

## Verification

```sh
cargo test
cargo test --test macro_sim
cargo build --bin mesopotamia
cargo build --bin demo1
bash smoke.sh
```

## Out of Scope

- Any behaviour change.
- Splitting other files (`grid.rs`, `render.rs`) — revisit only if they earn it.
- Service-layer lattice consolidation (overlaps `winding-rivers-and-channels`).

## Surface after this phase

- `elk.rs` + `elk/{components,spawn,movement,metabolism,color}.rs`; external paths preserved
  via `pub use` in the root.
- `doc03.01.02` reflects the `elk/` layout.
- Negative space: simulation behaviour is byte-for-byte the same as before the split.
