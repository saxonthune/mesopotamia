# Macro-state test harness (headless soak run)

## Motivation

The sim has unit/metamorphic tests on pure metrics (`field::`, `elk::cross_desire`) but
nothing that verifies *emergent* behaviour — that a full run stays alive and sane. This
task builds a headless `App` that steps the simulation deterministically and asserts
macro-state invariants. It is the first instance of the `/verification` skill's top rung
(a seeded soak run, the grand-strategy "observer mode" move): the elk already play
themselves, so we run them forward and check the world doesn't collapse or explode.

This is the **collision-free** piece — a new lib target + a new `tests/` file. It ships
independently of the structural reorg (the two follow-on tasks) and of the in-flight
`winding-rivers-and-channels` / `water-as-barrier-fords` chains.

## Do NOT

- **Do NOT change any simulation behaviour.** No tuning, no new systems, no balance edits.
  This is structure (a lib target) + a test. The only new sim-facing code allowed is
  **read-only** metric helpers (`fn total_grass(&Grid) -> f32`, etc.).
- **Do NOT add `RenderPlugin` or `UiPlugin`** to the harness `App`. The whole point is that
  the sim loads headless. `ElkPlugin`/`GridPlugin`/`RiverPlugin` may stay whole even though
  `ElkPlugin` still bundles its two render-sync systems — under `MinimalPlugins` those run
  as harmless no-ops (elk carry `Sprite`+`Transform`; nothing draws them). Splitting them
  out is a *later* task; do not do it here.
- **Do NOT rely on wall-clock time** to advance the fixed timestep. Drive it manually with
  `TimeUpdateStrategy::ManualDuration` so the run is reproducible (see Plan step 3).
- **Do NOT touch `src/bin/demo1.rs`.** It keeps its `#[path]` includes and compiles
  independently of the new lib target.
- **Do NOT assert bit-determinism** (two runs identical). A single-machine sim needs
  reproducibility, not lockstep determinism. The fixed seed already gives reproducibility.

## Plan

### 1. Add a library target (`src/lib.rs`)

`tests/` integration tests can only import a *library* crate, and none exists today (both
`main.rs` and `demo1.rs` are binaries). Create `src/lib.rs` that re-exports the modules:

```rust
pub mod field;
pub mod grid;
pub mod elk;
pub mod render;
pub mod ui;
pub mod river;
```

Cargo auto-detects `src/lib.rs` as the `mesopotamia` lib target — no `Cargo.toml` change.

### 2. Point `main.rs` at the lib (so modules aren't double-compiled)

In `src/main.rs`, remove the `mod field; mod grid; …` declarations and the `use crate::…`
lines, and instead bring the plugins in from the library crate:

```rust
use mesopotamia::grid::GridPlugin;
use mesopotamia::elk::ElkPlugin;
use mesopotamia::ui::UiPlugin;
use mesopotamia::render::RenderPlugin;
use mesopotamia::river::RiverPlugin;
```

The body of `main()` is unchanged. Confirm `cargo run --bin mesopotamia` still builds.
(`demo1.rs` is untouched — it keeps its own `#[path]` module tree.)

### 3. Write the headless harness (`tests/macro_sim.rs`)

Build an `App` with `MinimalPlugins` (which includes `TimePlugin`) plus the three sim
plugins, and step it a fixed number of ticks deterministically. The reproducible-stepping
recipe — the technical crux the draft flagged:

```rust
use bevy::prelude::*;
use bevy::app::ScheduleRunnerPlugin;
use bevy::time::TimeUpdateStrategy;
use core::time::Duration;
use mesopotamia::{grid::GridPlugin, elk::ElkPlugin, river::RiverPlugin};

const HZ: f64 = 10.0;
const PERIOD: Duration = Duration::from_millis(100); // 1000ms / 10Hz

fn headless(ticks: u32) -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins.set(ScheduleRunnerPlugin::run_once()))
        .insert_resource(Time::<Fixed>::from_hz(HZ))
        // Each app.update() advances virtual time by exactly one fixed period,
        // so exactly one FixedUpdate runs per update — reproducible, no wall clock.
        .insert_resource(TimeUpdateStrategy::ManualDuration(PERIOD))
        .add_plugins((GridPlugin, ElkPlugin, RiverPlugin));
    for _ in 0..ticks {
        app.update();
    }
    app
}
```

If `MinimalPlugins.set(ScheduleRunnerPlugin::run_once())` doesn't typecheck against 0.18.1,
fall back to plain `MinimalPlugins` — the loop drives stepping regardless; the runner plugin
is only to stop it from spinning. Verify the exact 0.18.1 API and adjust.

### 4. Extract read-only macro metrics

Pure helpers, defined in the test file (or as `pub fn` on `Grid` if cleaner — your call,
but prefer keeping them test-side to avoid touching sim files):

- `total_grass(grid: &Grid) -> f32` — sum of `grid.grass(i)` over all cells.
- `elk_count(app: &mut App) -> usize` — count `Elk` entities (`world.query::<&Elk>()`).
- `max_col_reached(app: &mut App, grid: &Grid) -> usize` — highest column index any elk
  currently occupies (via `grid.col_row(elk.cell)`), the "how far has the journey gone"
  signal for the migration invariant.

### 5. Assert the three invariants

Establish a **baseline first**: in development, run the harness for a long horizon (e.g.
3000 ticks) and `println!` the metrics each few hundred ticks (run with `--nocapture`).
Read the stable values, then set envelopes with generous margin — these are
collapse/explosion guards, not tight specs.

- **`population_does_not_collapse_or_explode`** — after a warmup + run, `elk_count` is `> 0`
  and below a sane ceiling (e.g. `< 8 * TARGET_POPULATION`). Pin the band around the
  observed steady state.
- **`grass_never_fully_collapses`** — track `min` of `total_grass` across the whole run;
  assert it stays above a small floor (the ecology never gets eaten to bare dirt).
- **`herds_reach_the_far_edge`** — over a long run, `max_col_reached` reaches into the
  `EDGE_COL` band (the journey actually crosses the map). If no clean read-only observable
  exists for "an elk migrated off", asserting that elk *reach* the edge band is sufficient;
  do NOT add a migration counter to the sim (that's a behaviour change).

Use a fixed tick horizon so the test is reproducible. Keep total test runtime reasonable
(a few seconds); if 3000 ticks is too slow under the dev profile, trim to the smallest
horizon that still exercises a steady state and document the number.

## Files to Modify

- `src/lib.rs` — NEW. `pub mod` re-exports of the six modules.
- `src/main.rs` — swap local `mod`/`use crate::` for `use mesopotamia::…`.
- `tests/macro_sim.rs` — NEW. The headless harness + three invariant tests + metric helpers.

## Verification

```sh
cargo build --bin mesopotamia
cargo build --bin demo1
cargo test --test macro_sim
cargo test
bash smoke.sh --no-run
```

## Out of Scope

- Sim/render plugin split (separate task, sequenced after the fording chain).
- `elk.rs` module split (separate task).
- Service-layer consolidation of lattice algorithms (overlaps `winding-rivers-and-channels`).
- Tightening the invariant envelopes into precise specs — start loose; tighten when a real
  regression motivates it (the `/verification` skill's "start sparse" principle).

## Notes

- The `/verification` skill (`.claude/skills/verification/SKILL.md`) is the conceptual
  backing: this harness is its "seeded soak run" rung. Read it before tuning the asserts.
- Reproducibility, not bit-determinism, is the goal — see that skill's determinism note.

## Surface after this phase

- `src/lib.rs` exists and is the `mesopotamia` library target, exposing
  `pub mod {field, grid, elk, render, ui, river}`. Downstream tasks import sim plugins via
  `mesopotamia::elk::…` etc.
- `tests/macro_sim.rs` exists with a `headless(ticks: u32) -> App` helper and three passing
  invariant tests. Later tasks (sim/render split) MUST keep these green.
- `src/main.rs` consumes the lib crate rather than re-declaring modules.
- Negative space: `ElkPlugin` is still a single plugin bundling sim + render-sync systems —
  the split has NOT happened yet. `demo1.rs` still uses `#[path]` includes and is unchanged.
