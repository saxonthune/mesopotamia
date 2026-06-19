# Act Palette Plumbing (Phase 1 of elk-decision-palette)

## Motivation

The elk decision model (doc03.01.08) reframes an elk's per-tick choice as a flat
palette of candidate next-states — four moves plus *stand* and *graze* — each
scored in one currency and picked by a Boltzmann softmax, with eating gated on
having chosen to graze. This phase lays the **data substrate** for that model
with **zero behavior change**: introduce an `Act` enum, generalize the decision
record from "directional steps" to "scored candidates," and thread a `chosen_act`
through. Behavior stays byte-identical (still only the four `Step` candidates,
eating still unconditional). Splitting the type churn out first de-risks Phase 2's
behavioral change and keeps the diff bisectable.

## Do NOT

- **Do NOT change any behavior.** No Stand/Graze candidates are *produced* yet, no
  graze gating, no new scoring. `just test-fast` must stay green with no test logic
  changed (only mechanical renames in test/harness code if a symbol moved).
- **Do NOT touch** `metabolism.rs` eating logic, `ElkParams`, `EnergyFlows`/`ledger.rs`,
  `combine_drives`, `Drives`, `grass_gradient`, `forage_gate`, `migration_residual`,
  `step_water_penalty`, or `cross_desire`/`forage_across`.
- **Do NOT** overload `Elk::prev_cell` (it is render-only) to signal anything.
- **Do NOT** change the FixedUpdate system order in `mod.rs`.
- **Do NOT** widen the verification beyond `just test-fast`.

## Plan

### 1. Add the `Act` enum (`src/elk/movement.rs`)

Near `StepEval`/`Decision` at the top of the file, add:

```rust
/// What an elk commits to this tick. Step travels to a cardinal neighbour;
/// Stand and Graze both hold position (Graze additionally feeds, in Phase 2).
/// Stored as a field on the decision record — never a marker component — so an
/// elk's archetype never churns (see doc03.01.07).
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Act {
    Step(isize, isize),
    Stand,
    Graze,
}
```

### 2. Generalize `StepEval` → `Candidate` (`src/elk/movement.rs`)

Rename the struct `StepEval` to `Candidate` and replace its `step: (isize, isize)`
field with `act: Act`. Keep `score`, `penalty`, `weight` unchanged. Update the
doc comment to "One candidate action evaluated by the softmax decision."

### 3. Add `chosen_act` to `Decision` (`src/elk/movement.rs`)

Add `pub chosen_act: Act` to `Decision`. In `Decision::default()` set
`chosen_act: Act::Stand` (the safe "do nothing" default for a hemmed-in elk).
`options` becomes `Vec<Candidate>`; `chosen: Option<usize>` is unchanged.

### 4. Update `herd_move` to emit `Candidate`s and set `chosen_act` (`src/elk/movement.rs:204-421`)

- Where options are built (around `:354-364`), construct `Candidate { act: Act::Step(dx, dy), score, penalty, weight }`. Still only the four `Step` candidates — do not add Stand/Graze here.
- After the softmax pick sets `elk.cell = next` (`:377-386`), record which act was chosen: set a local `chosen_act` to `Act::Step(dx, dy)` for the picked step, defaulting to `Act::Stand` if nothing was picked.
- Write `chosen_act` into the `Decision` you assemble at `:388-393`.
- In the hemmed-in branch (`:336-345`), set `chosen_act: Act::Stand`.
- Movement is otherwise unchanged: `elk.cell = next` still happens exactly as today.

### 5. Update the harness decider (`src/sim_harness.rs:243-274`)

`FieldDecider::decide` builds `StepEval`/`Decision` by hand and **mirrors**
`herd_move`. Update it in lockstep: push `Candidate { act: Act::Step(dx, dy), .. }`,
and set `chosen_act` on the returned `Decision` (from the best step, or `Act::Stand`
when nothing is finite). Import `Act` alongside the existing `StepEval`→`Candidate`
import on `src/sim_harness.rs:8`.

### 6. Update the UI decision panel (`src/ui.rs:670-708`)

`elk_decision_panel` renders each option with `step_arrow(eval.step)`. Replace that
with a glyph derived from `eval.act`. Add a small helper:

```rust
fn act_glyph(act: Act) -> String {
    match act {
        Act::Step(dx, dy) => step_arrow((dx, dy)),
        Act::Stand => "■".to_string(),   // hold position
        Act::Graze => "🌿".to_string(),  // feed in place
    }
}
```

Handle all three variants now (Stand/Graze never appear in Phase 1, but wiring them
here means Phase 2 does not reopen `ui.rs`). Keep `step_arrow` as-is for the `Step`
case. Import `Act` in `ui.rs`.

### 7. Fix remaining references

Update every `StepEval` reference to `Candidate` and every `.step` access on a
candidate to `.act` (or destructure the `Act::Step`). Re-export `Act` from the elk
module: add it to the `pub use movement::{…}` list in `src/elk/mod.rs:18`.

## Files to Modify

- `src/elk/movement.rs` — add `Act`, rename `StepEval`→`Candidate`, add `Decision.chosen_act`, set it in `herd_move`.
- `src/elk/mod.rs` — re-export `Act` (and the renamed `Candidate`).
- `src/sim_harness.rs` — update `FieldDecider::decide` to `Candidate`/`Act` + `chosen_act`.
- `src/ui.rs` — `act_glyph` helper; render candidates by `act`.
- Any test referencing `StepEval` — mechanical rename only (no logic change).

## Verification

```bash
just test-fast
```

All existing tests must pass with no assertion logic changed. The build must be
clean across the library crate (`cargo test --lib` compiles `sim_harness`, `ui`,
and `render`).

## Out of Scope

- Stand/Graze candidate *production*, value scoring, graze gating, tuning — all Phase 2.
- Any energy-ledger or `ElkParams` change.

## Notes

- This phase is a refactor: if `just test-fast` shows any behavioral test changed,
  something leaked. The only legitimate test edits are `StepEval`→`Candidate` renames.
- `Act::Step` keeps `isize` (not `i8`) to match `grid.step(dx, dy)` and avoid casts.

## Surface after this phase

- `Act` enum exported from the `elk` module: `Act::Step(isize, isize) | Act::Stand | Act::Graze`, deriving `Clone, Copy, PartialEq, Debug`.
- `Candidate` struct (formerly `StepEval`), exported: fields `act: Act`, `score: f32`, `penalty: f32`, `weight: f32`.
- `Decision` struct: `drives: Drives`, `options: Vec<Candidate>`, `chosen: Option<usize>`, `chosen_act: Act`, `temperature: f32`. `Decision::default()` has `chosen_act = Act::Stand`.
- `herd_move` writes `chosen_act`: `Act::Step(dx, dy)` for a move, `Act::Stand` when hemmed in. It still produces **only** `Step` candidates (≤4) and still moves via `elk.cell = next` exactly as before.
- `FieldDecider::decide` (harness) produces the same `Candidate`/`chosen_act` shape, deterministically.
- `ui::act_glyph` renders all three `Act` variants; `ui.rs` does not need reopening in Phase 2.
- **Negative space (relied on by Phase 2):** `metabolism::graze` is UNCHANGED — it still feeds every elk unconditionally, ignoring `chosen_act`. `ElkParams`, `EnergyFlows`/`ledger.rs`, `combine_drives`, `Drives`, `grass_gradient`, `forage_gate` are all UNCHANGED. The FixedUpdate order (`herd_move` → `graze` → `metabolize` → …) is UNCHANGED.
