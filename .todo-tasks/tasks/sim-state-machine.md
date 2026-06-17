# Sim state machine (Phase 1 of world-regenerate)

## Motivation

World generation currently runs once on `Startup` (`worldgen/mod.rs`), with no way
to tear the world down and rebuild it at runtime. This phase introduces a Bevy
`States`-based `Sim` state machine (`Generating` → `Running`) as the foundation
for runtime regeneration, **without changing observable behavior**: at boot the
app still generates exactly one world and then runs the sim, identically to today.
Phase 2 (`regenerate-control`) builds the actual teardown + UI trigger on top of
the Surface this phase leaves behind.

This builds on the just-landed seed-threading: `WorldSeed` is already a resource
inserted by `WorldgenPlugin`, and `generate_world` already reads it.

## Do NOT

- Do NOT implement any regenerate trigger, UI button, elk despawn, or resource
  teardown — that is Phase 2. This phase only wires the state machine and keeps
  boot behavior identical.
- Do NOT change `ElkParams`, `GrowthRate`, `Fertility`, or any tunable defaults.
- Do NOT use `StateScoped` / `DespawnOnExit` components in this phase (no entities
  are torn down yet).
- Do NOT alter the worldgen layer algorithms or the seed derivation — only move
  *when* `generate_world` runs.
- Do NOT add the `Sim` state only to one binary. Both `main.rs` (lib-crate
  consumer) and `src/bin/demo1.rs` (`#[path]` consumer) must register it, and so
  must the headless harness `sim_harness.rs`.

## Plan

### 1. New shared module `src/sim.rs`

Create `src/sim.rs` exposing the state enum and a plugin that registers it:

```rust
use bevy::prelude::*;

/// The lifecycle of the simulation. `Generating` runs world generation; `Running`
/// steps the sim. Regeneration (Phase 2) is a transition back to `Generating`.
#[derive(States, Default, Clone, Eq, PartialEq, Hash, Debug)]
pub enum Sim {
    /// Default at boot: world generation runs in `OnEnter(Sim::Generating)`.
    #[default]
    Generating,
    /// The sim steps: grid growth + elk systems run, gated on this state.
    Running,
}

/// Registers the `Sim` state. Added by every binary and the headless harness so
/// the same state path runs in app and test contexts.
pub struct SimStatePlugin;

impl Plugin for SimStatePlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<Sim>();
    }
}
```

Add `pub mod sim;` to `src/lib.rs` (it lists the modules alphabetically-ish; place
it near `settings`/`render` — exact position is not load-bearing).

### 2. Register the plugin in both binaries

- `src/main.rs`: import `use mesopotamia::sim::SimStatePlugin;` and add
  `SimStatePlugin` to the `.add_plugins((...))` tuple alongside the others.
- `src/bin/demo1.rs`: add the `#[path]` include next to the existing ones:
  `#[path = "../sim.rs"] mod sim;`, then `use crate::sim::SimStatePlugin;` and add
  `SimStatePlugin` to its `.add_plugins((...))` tuple.

`crate::sim::Sim` resolves in both builds (lib module for `main.rs`, top-level
`mod sim` for `demo1.rs`), so downstream `use crate::sim::Sim` works in shared modules.

### 3. Move worldgen onto `OnEnter(Sim::Generating)` and transition to `Running`

In `src/worldgen/mod.rs`:

- `WorldgenPlugin::build`: keep `app.insert_resource(WorldSeed(rand::random()))`,
  but change `.add_systems(Startup, generate_world)` to
  `.add_systems(OnEnter(Sim::Generating), generate_world)`.
- `generate_world`: add `mut next: ResMut<NextState<Sim>>` to the params. At the
  **start**, call `grid.reset()` (see step 4) so the function is re-entrant (a
  second entry rebuilds onto a clean grid). At the **end**, `next.set(Sim::Running)`.
- Import `use crate::sim::Sim;`.

At boot, `Generating` is the default state, so `OnEnter(Sim::Generating)` fires
once during the first state-transition pass → one world is generated → transition
to `Running`. Net boot behavior matches today.

### 4. `Grid::reset()` in `src/grid.rs`

Add a pure method that returns every cell field to its `Grid::new` default without
changing dimensions:

```rust
/// Clear all per-cell state back to a freshly-constructed world, preserving
/// dimensions. World generation calls this before re-stamping, so regeneration
/// starts from a clean substrate instead of layering onto the previous world.
pub fn reset(&mut self) {
    *self = Self::new(self.width, self.height);
}
```

Add a unit test in `grid.rs`'s `#[cfg(test)] mod tests` asserting that after
mutating some cells (e.g. `set_grass`, `add_poop`, `set_water`) then `reset()`,
the grid equals a fresh `Grid::new` (e.g. `total_grass() == 0.0`, a sampled
`water`/`poop`/`soil` cell back to defaults: water 0, poop 0, soil 1.0,
water_prox 1.0).

### 5. Gate the sim systems on `in_state(Sim::Running)`

- `src/grid.rs` `GridPlugin::build`: change the FixedUpdate registration to
  `.add_systems(FixedUpdate, (growth, grow_shrubs, fertilize).run_if(in_state(Sim::Running)))`.
  Import `use crate::sim::Sim;`.
- `src/elk/mod.rs` `ElkSimPlugin::build`: apply `.run_if(in_state(Sim::Running))`
  to the FixedUpdate tuple (`herd_move, graze, digest, metabolize, migrate_pressure,
  spawn_waves, cull`). Leave `tally_herds` on `Update` ungated (it is a harmless UI
  feeder and a no-op when no elk exist). Import `use crate::sim::Sim;`.

### 6. Update the headless harness `src/sim_harness.rs`

`make_app()` must register the state so worldgen-on-enter fires and the gated sim
systems run. Add `SimStatePlugin` to its `.add_plugins((...))`:
`(GridPlugin, ElkSimPlugin, WorldgenPlugin, SimStatePlugin)` and
`use crate::sim::SimStatePlugin;`.

Because worldgen now runs in `OnEnter(Generating)` and the sim starts after the
transition to `Running`, the harness reaches `Running` within the first couple of
`app.update()` calls (boot state is `Generating`; `generate_world` sets
`NextState(Running)`). The `macro_sim`/`balance` tests run hundreds of ticks and
assert ranges/invariants, not exact per-tick counts, so a one-update startup
offset is within tolerance. If the tests reveal that the world never generates or
the sim never steps under `MinimalPlugins`, the cause is the `StateTransition`
schedule not being wired — add `app.add_plugins(StatesPlugin)` explicitly in
`make_app` before `init_state` resolves it. Verify by the tests passing, not by
assumption.

## Files to Modify

- `src/sim.rs` — NEW: `Sim` state enum + `SimStatePlugin`.
- `src/lib.rs` — add `pub mod sim;`.
- `src/main.rs` — register `SimStatePlugin`.
- `src/bin/demo1.rs` — `#[path]` include `sim`, register `SimStatePlugin`.
- `src/worldgen/mod.rs` — `Startup` → `OnEnter(Sim::Generating)`; `generate_world`
  resets the grid first and sets `NextState(Running)`.
- `src/grid.rs` — `Grid::reset()` + unit test; gate FixedUpdate systems on `Running`.
- `src/elk/mod.rs` — gate FixedUpdate systems on `Running`.
- `src/sim_harness.rs` — add `SimStatePlugin` to `make_app`.

## Verification

```bash
cargo build --bins
just test-fast
cargo test --test macro_sim
cargo test --test balance
```

All four must pass. `test-fast` covers the new `Grid::reset` unit test and the
existing pure-function suite; `macro_sim` and `balance` confirm the headless
harness still generates a world and steps the sim through the new state path.

## Out of Scope

- The regenerate trigger, UI button, elk/resource teardown (Phase 2).
- Any pause/menu states beyond `Generating`/`Running`.

## Notes

- Bevy 0.17 API: states use `#[derive(States, ...)]` + `app.init_state::<Sim>()`;
  gating uses `.run_if(in_state(Sim::Running))`; transitions use
  `ResMut<NextState<Sim>>` + `next.set(...)`. `OnEnter(state)` fires for the
  default state once at startup.
- The `in_state` run condition needs the `State<Sim>` resource to exist; that is
  exactly why every app/harness that runs these plugins must add `SimStatePlugin`.
  This is the one cross-cutting risk — the harness change in step 6 is not optional.

## Surface after this phase

- `src/sim.rs` exists and is public (`mesopotamia::sim` / `crate::sim`), exposing:
  - `pub enum Sim { Generating (default), Running }` deriving `States`.
  - `pub struct SimStatePlugin` whose `build` calls `init_state::<Sim>()`.
- `SimStatePlugin` is registered in `main.rs`, `src/bin/demo1.rs`, and
  `sim_harness::make_app`.
- `WorldgenPlugin` runs `generate_world` on `OnEnter(Sim::Generating)` (no longer
  `Startup`); `generate_world` calls `Grid::reset()` first and ends by setting
  `NextState(Sim::Running)`. `WorldSeed` is still inserted by the plugin with a
  random per-run value.
- `Grid::reset(&mut self)` exists: clears all per-cell state to `Grid::new`
  defaults, preserving dimensions. Unit-tested.
- All `GridPlugin` and `ElkSimPlugin` `FixedUpdate` systems are gated with
  `run_if(in_state(Sim::Running))`. `tally_herds` remains on `Update`, ungated.
- Negative space / not yet present (Phase 2 will add): no UI "regenerate" control;
  no `OnExit(Sim::Running)` despawn of elk; elk resources (`Spawner`, `Herds`,
  `Packs`, `DriveSamples`) are NOT reset on re-entering `Generating` — so a second
  entry into `Generating` rebuilds the grid but would leave stale elk/resources.
  Entering `Generating` a second time is not yet triggered anywhere.
