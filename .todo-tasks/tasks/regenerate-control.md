# Regenerate control (Phase 2 of world-regenerate)

## Motivation

Phase 1 (`sim-state-machine`) wired a `Sim` state machine (`Generating` →
`Running`) and made `generate_world` re-entrant (it resets the grid on every
`OnEnter(Sim::Generating)`). This phase makes regeneration actually usable: tear
down the live elk + their lifecycle resources when leaving `Running`, and add a UI
control that draws a fresh `WorldSeed` and transitions back to `Generating` — so
the user can roll a new world and restart the sim without reloading the app or the
web page.

## Triage basis (Phase 1 Surface)

This phase triages against Phase 1's declared Surface, not live code:

- `crate::sim::Sim` is a `States` enum with `Generating` (default) and `Running`;
  `SimStatePlugin` (`init_state::<Sim>()`) is registered in `main.rs`,
  `src/bin/demo1.rs`, and `sim_harness::make_app`.
- `WorldgenPlugin` runs `generate_world` on `OnEnter(Sim::Generating)`;
  `generate_world` calls `Grid::reset()` first and ends with `NextState(Sim::Running)`.
  `WorldSeed(pub u64)` is a resource inserted by `WorldgenPlugin`.
- `Grid::reset(&mut self)` clears per-cell state to `Grid::new` defaults.
- All `GridPlugin` / `ElkSimPlugin` `FixedUpdate` systems are gated on
  `run_if(in_state(Sim::Running))`; `tally_herds` is on `Update`, ungated.
- Not yet present (this phase adds): no UI regenerate control; no
  `OnExit(Sim::Running)` teardown; elk resources (`Spawner`, `Herds`, `Packs`,
  `DriveSamples`) are NOT reset on re-entering `Generating`.

## Do NOT

- Do NOT reset the tuned slider resources `ElkParams`, `GrowthRate`, or
  `Fertility` on regenerate — the user keeps their tuning across a new world. Only
  world (grid) + elk population/lifecycle state resets.
- Do NOT reset the grid in this phase's teardown — `generate_world` already calls
  `Grid::reset()` on `OnEnter(Generating)` (Phase 1 Surface). Adding a second reset
  is redundant; rely on the existing one.
- Do NOT re-spawn elk explicitly after regenerate — `spawn_waves` repopulates
  naturally once `Running` resumes and `Spawner` is back at its default (so the
  first wave fires immediately). Do not add an initial-spawn system.
- Do NOT change the `Sim` enum, the worldgen trigger, or the system gating from
  Phase 1.
- Do NOT use a fixed seed for the regenerate button — draw a fresh random one each
  click so each regenerate is a different world (mirrors the per-run default).

## Plan

### 1. Teardown on `OnExit(Sim::Running)` — `src/elk/spawn.rs`

Add a single `teardown` system that despawns all elk and resets their lifecycle
resources (the explicit-despawn approach chosen over `StateScoped`):

```rust
/// On leaving `Running` (a regenerate), despawn every elk and reset the herd
/// lifecycle resources so the new world starts from an empty range. The tuned
/// `ElkParams` are deliberately left untouched.
pub(super) fn teardown(
    mut commands: Commands,
    elk: Query<Entity, With<Elk>>,
    mut spawner: ResMut<Spawner>,
    mut herds: ResMut<Herds>,
    mut packs: ResMut<Packs>,
    mut drive_samples: ResMut<DriveSamples>,
) {
    for e in &elk {
        commands.entity(e).despawn();
    }
    *spawner = Spawner::default();
    *herds = Herds::default();
    *packs = Packs::new();
    drive_samples.per_slot = vec![DriveSample::default(); super::PACK_COUNT];
}
```

Use whatever imports the module already has for these types (`Spawner`, `Herds`,
`Packs`, `DriveSamples`, `DriveSample`, `Elk` from `super::components`).

Register it in `src/elk/mod.rs` `ElkSimPlugin::build`:
`.add_systems(OnExit(Sim::Running), spawn::teardown)` and
`use crate::sim::Sim;` (already imported in Phase 1 — reuse it).

At boot the app goes `Generating → Running` and never exits `Running`, so
`teardown` only ever runs on a real regenerate. The transition order on regenerate
is `OnExit(Running)` (this teardown) → `OnEnter(Generating)`
(`generate_world` → grid reset + rebuild → `NextState(Running)`), so the grid is
rebuilt after the elk are cleared.

### 2. Regenerate button — `src/ui.rs` `control_panel`

Add two params to `control_panel` (it already has
`#[allow(clippy::too_many_arguments)]`):
`mut world_seed: ResMut<crate::worldgen::WorldSeed>` and
`mut next_state: ResMut<NextState<crate::sim::Sim>>`.

Place the control in the existing tab-bar `ui.horizontal(...)` row (next to
`speed_inline`), so it is always visible and avoids the declarative `Item::Custom`
borrow dance:

```rust
ui.separator();
if ui.button("⟳ regenerate").on_hover_text(format!("seed {:#018x}", world_seed.0)).clicked() {
    world_seed.0 = rand::random();
    next_state.set(crate::sim::Sim::Generating);
}
```

Add `use rand::Rng;` only if needed (`rand::random()` is a free function and does
not need the trait in scope). Keep the rest of `control_panel` unchanged.

### 3. Document the sim state — `.rhidoc/03-milestones/01-grazers/02-structure.md`

Add a short `## Sim State` subsection (present-tense, declarative — no "now",
"will", version/phase language). State that: a `Sim` state (`Generating` →
`Running`) lives in `src/sim.rs`; world generation runs in
`OnEnter(Sim::Generating)` and hands off to `Running`; grid and elk `FixedUpdate`
systems are gated on `Running`; a regenerate draws a fresh `WorldSeed` and
transitions back to `Generating`, which clears the grid (worldgen) and despawns
the elk + resets their lifecycle resources (`OnExit(Running)`), preserving the
tuned `ElkParams`/`GrowthRate`/`Fertility`. Keep it to a few lines; the `## Schedules`
subsection may gain one sentence noting the state-gating.

### 4. Headless regenerate test

Add an integration test (extend `tests/macro_sim.rs` or a small new
`tests/regenerate.rs` built on `crate::sim_harness`-style setup) that asserts the
teardown contract deterministically:

- Build a headless app (MinimalPlugins + `GridPlugin, ElkSimPlugin, WorldgenPlugin,
  SimStatePlugin`, fixed time), step until at least one elk exists and `Spawner.elapsed > 0`.
- Set `NextState(Sim::Generating)` and bump `WorldSeed` to a different value, then
  `app.update()` enough times for the transition to apply.
- Assert: immediately after `OnExit(Running)` applies, elk entity count is 0 and
  `Spawner.elapsed == 0` / `Herds::default()`-equivalent (empty `cohorts`).
- Assert the world regenerated from the new seed: a coarse grid signature (e.g.
  total water `Σ grid.water(i)` or the set of `is_ford` cells) differs from the
  pre-regenerate signature. If transition timing makes the exact frame fiddly,
  drive a few extra `update()`s and assert the steady-state invariant (elk
  repopulating from a fresh `Spawner`, world matches the new seed) rather than a
  single-frame snapshot — pin the intent, not the frame.

If `sim_harness::make_app` is convenient to reuse, expose a small helper there for
manually driving the state; otherwise inline the app setup in the test.

## Files to Modify

- `src/elk/spawn.rs` — `teardown` system (despawn elk + reset lifecycle resources).
- `src/elk/mod.rs` — register `teardown` on `OnExit(Sim::Running)`.
- `src/ui.rs` — `control_panel` gains the regenerate button + the two resource params.
- `.rhidoc/03-milestones/01-grazers/02-structure.md` — `## Sim State` subsection.
- `tests/macro_sim.rs` (or new `tests/regenerate.rs`) — headless regenerate test.

## Verification

```bash
cargo build --bins
just test-fast
cargo test --test macro_sim
cargo test --test balance
cargo test --test regenerate
```

The `regenerate` line applies only if a new test file is added; if the test is
folded into `macro_sim`, drop that line. The build must succeed for both binaries
(the UI references `WorldSeed`/`Sim` in the lib build and the `#[path]` demo1 build).

## Out of Scope

- A seed-input text field or "replay this seed" UI — the button draws a random
  seed only. (A future task can add a pinned-seed entry.)
- Pause/menu states; confirmation dialogs before regenerate.
- Persisting or surfacing the seed beyond the log line and the button hover text.

## Notes

- The one subtlety is borrow/ordering in `control_panel`: keep the button in the
  tab-bar `horizontal` closure where `world_seed`/`next_state` are mutated inline,
  rather than inside a captured `Item::Custom` closure, to avoid fighting the
  declarative panel borrows.
- `teardown` runs only on `OnExit(Running)`, which never fires at boot — so it adds
  no startup cost and cannot interfere with the initial generate→run path.
- Manual smoke check after merge: run `cargo run --bin demo1`, click ⟳ regenerate,
  confirm a new river layout appears and herds repopulate; the log prints a new
  `worldgen master seed = 0x...` each click.

## Surface after this phase

- `OnExit(Sim::Running)` runs `elk::spawn::teardown`: despawns all `Elk` entities
  and resets `Spawner`/`Herds`/`Packs`/`DriveSamples` to fresh state; leaves
  `ElkParams`/`GrowthRate`/`Fertility` untouched.
- `control_panel` exposes a "⟳ regenerate" button that sets `WorldSeed` to a fresh
  random value and transitions `Sim` to `Generating`, driving a full
  teardown→regenerate→run cycle with no app/page reload.
- `doc03.01.02` documents the `Sim` state and the regenerate cycle.
- A headless test pins the teardown + regenerate contract.
