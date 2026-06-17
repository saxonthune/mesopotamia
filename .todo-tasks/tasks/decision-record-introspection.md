# Decision Record + Decomposable + Introspection Panel

## Motivation

The per-step decision is opaque: `herd_move` (`src/elk/movement.rs`, the phase-2 loop) scores four
candidate steps into local `scores`/`cells`/`weights` arrays and discards them the instant it picks
a cell. Nothing keeps the per-agent, per-candidate reasoning. This is the keystone rung of the
observability ladder (doc02.03) and targets the elk-won't-cross-water bug directly: a selected
elk's score breakdown distinguishes "chose not to cross" (penalty > desire) from "couldn't perceive
a reason to" (desire ≈ 0), which look identical in every aggregate view.

## Do NOT

- Do NOT change the movement math or the softmax pick. Behaviour must be identical — same cell
  chosen for the same RNG state. Build the `Decision` from the values already computed; do not draw
  from `rng` any earlier or in a different order than today (`rng.random_range` stays the single
  draw per elk, after weights are computed).
- Do NOT remove or alter `DriveSamples` / the per-slot drive pie — this phase adds alongside it.
- Do NOT add field overlays, histograms, event logs, or harness probes (later phases).
- Do NOT touch `combine_drives`' signature or the existing metamorphic tests.

## Plan

### 1. `Decomposable` trait + impl for `Drives` (`src/elk/movement.rs`)

Add a trait exposing a labeled view of a weighted decomposition, and impl it for `Drives` (keep all
typed fields and `total`/`migration_share`):

```rust
pub trait Decomposable {
    fn contributions(&self) -> Vec<(&'static str, Vec2)>;
}
```

`Drives::contributions` yields `[("sep", self.sep), ("coh", self.coh), ("grass", self.grass),
("social", self.social), ("migration", self.migration)]`. Unit-test that the labels/order match the
existing `DRIVE_COLORS` order in `ui.rs` and that `total()` equals the vector sum of contributions.

### 2. `Decision` record (`src/elk/movement.rs`)

Add plain-data types capturing one elk's tick decision:

```rust
pub struct StepEval { pub step: (isize, isize), pub score: f32, pub penalty: f32, pub weight: f32 }
pub struct Decision { pub drives: Drives, pub options: Vec<StepEval>, pub chosen: Option<usize>, pub temperature: f32 }
```

`chosen` is the index into `options` that was picked (None if hemmed in).

### 3. Build the Decision in `herd_move` (`src/elk/movement.rs`)

In the phase-2 loop, after the existing `scores`/`cells`/`weights` computation, assemble a
`Decision` (one `StepEval` per valid step, recording raw `score`, the `step_water_penalty` value,
and the softmax `weight`). Keep the existing pick loop; record which index won as `chosen`. Store
the result on the elk via a new component (step 4). No behavioural change.

### 4. `LastDecision` component (`src/elk/components.rs`)

Add `#[derive(Component)] pub struct LastDecision(pub Decision);` Insert/overwrite it on each elk at
the end of `herd_move` (use `Commands` or a mutable component already on the elk — prefer adding the
component once at spawn and mutating it, to avoid per-tick archetype moves). Export `Decision`,
`StepEval`, `Decomposable`, `LastDecision` from `src/elk/mod.rs` alongside `Drives`.

### 5. Per-elk selection (`src/ui.rs`)

`UiState.selected` is a herd `code` only. Add `selected_elk: Option<Entity>` to `UiState`. In
`pick_herd`, the nearest-elk search already finds the best elk — capture its `Entity` and set
`state.selected_elk` alongside `state.selected`.

### 6. Introspection panel (`src/ui.rs`)

In `herd_details` (or a new section within the Herds tab), when `state.selected_elk` resolves to an
elk with a `LastDecision`, render: the drive decomposition (reuse the pie / `Item::Pie`, driven by
`Decomposable::contributions`), and a small table of the four `StepEval`s — step direction, score,
water penalty, and softmax probability (`weight / Σweight`), with the chosen one marked. Query
`LastDecision` read-only. Degrade to a "no decision yet" label when absent.

## Files to Modify

- `src/elk/movement.rs` — `Decomposable`, `Decision`/`StepEval`, build the record in `herd_move`, unit tests.
- `src/elk/components.rs` — `LastDecision` component.
- `src/elk/mod.rs` — export the new symbols.
- `src/elk/spawn.rs` — insert a default/empty `LastDecision` at elk spawn (to mutate in place).
- `src/ui.rs` — `selected_elk` field, capture in `pick_herd`, render the panel.

## Verification

```bash
just test-fast
cargo build
```

## Out of Scope

- Sampling drives at empty cells / field overlays (phase 2).
- Metric registry, event log, conservation ledger, probes (later phases).

## Notes

- Determinism is the headline risk: `run_metrics`/the balance sweep (doc02.02, `sim_harness.rs`) and
  the metamorphic tests depend on the exact RNG draw order in `herd_move`. Verify `just test-all`
  behaviour is unchanged if in doubt, but the gate above is `test-fast` + build for speed.
- Elk systems run `.run_if(in_state(Sim::Running))`; the panel reads whatever the last Running tick
  wrote, which is correct.
- Patterns: doc02.01 (pure tested functions), doc02.03 (rung 1).

## Surface after this phase

- `pub trait Decomposable { fn contributions(&self) -> Vec<(&'static str, Vec2)>; }` in
  `src/elk/movement.rs`, impl'd for `Drives`, exported from `src/elk` (`crate::elk::Decomposable`).
- `pub struct Decision { drives: Drives, options: Vec<StepEval>, chosen: Option<usize>, temperature: f32 }`
  and `pub struct StepEval { step: (isize,isize), score: f32, penalty: f32, weight: f32 }`, exported
  from `crate::elk`.
- `pub struct LastDecision(pub Decision)` — a `Component` present on every spawned elk, overwritten
  each Running tick by `herd_move`. Exported from `crate::elk`.
- `UiState.selected_elk: Option<Entity>` in `src/ui.rs`, set by `pick_herd` to the picked elk.
- `herd_move` behaviour (cells chosen, RNG order, `DriveSamples` output) is UNCHANGED — later phases
  may rely on movement and the balance metrics being byte-identical to pre-this-phase.
- `combine_drives`, `Drives` fields/methods, `DriveSamples`, and the per-slot pie are unchanged and
  still exist.
