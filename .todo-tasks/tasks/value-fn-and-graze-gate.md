# Unified Value Function + Graze Gate (Phase 2 of elk-decision-palette)

## Motivation

With the `Act` palette substrate in place (Phase 1 Surface), this phase actualizes
the decision model from doc03.01.08: produce **six** candidates (four `Step` moves
plus `Stand` and `Graze`), score them in one shared currency, softmax over all six,
and make **eating require choosing `Graze`**. The result: an elk visibly *pauses*
to feed instead of stepping every tick, and a sated elk *rests* (`Stand`) instead
of pointlessly stripping grass. Scores stay in **desire-magnitude units** (the
pragmatic currency); a `dwell` tunable calibrates graze appeal against move scores.
Moves remain energy-free this pass (the ledger is untouched).

## Triage basis

This spec triages against Phase 1's `## Surface after this phase`, not live code.
Rely on: `Act::Step|Stand|Graze`, `Candidate { act, score, penalty, weight }`,
`Decision { …, chosen_act, … }`, `herd_move` already writing `chosen_act` and
producing only `Step` candidates, `metabolism::graze` still feeding unconditionally,
`ui::act_glyph` already handling all three variants.

## Do NOT

- **Do NOT** touch `EnergyFlows`/`ledger.rs` or add a movement energy cost. Moves stay free; only the existing basal `energy_drain` applies. The energy-conservation tests must not move.
- **Do NOT** change `forage_gate`'s signature or formula, or `combine_drives`/`Drives`/`grass_gradient`/`migration_residual`. The desire vector survives unrefactored as the *spatial term* of the value.
- **Do NOT** stop updating `intake_rate`/`HabitatIntake` for non-grazing elk — movers must still post a 0-intake tick so the gate signal stays well-defined (see step 3).
- **Do NOT** reopen `ui.rs` — Phase 1 already renders all `Act` variants.
- **Do NOT** widen verification beyond `just test-fast`.
- **Do NOT** express graze/stand scores in energy units — use desire-magnitude units (the `dwell`-calibrated form below).

## Plan

### 1. Pure scoring functions (`src/elk/movement.rs`)

Add two pure, tested functions next to the other pure helpers:

```rust
/// Value of grazing the current cell, in the same units as a move's
/// `desire·dir − penalty` score. Marginal value of intake declines with the
/// store: the `(1 − energy)` factor sends it to 0 at satiation, so a full elk is
/// indifferent between Graze and Stand. `here_forage` is `grid.forage(cell)`.
pub fn graze_value(here_forage: f32, energy: f32, dwell: f32) -> f32 {
    dwell * (1.0 - energy).clamp(0.0, 1.0) * here_forage.max(0.0)
}

/// Value of standing still — the conserve baseline. Holding position is the zero
/// reference a move must beat (a move scores `desire·dir − penalty`, which is 0
/// when the desire vector is orthogonal to the step and there's no penalty).
pub fn stand_value() -> f32 {
    0.0
}
```

Add unit tests in the file's `#[cfg(test)]` module:
- `graze_value` is 0 when `energy == 1.0` regardless of forage (satiation indifference).
- `graze_value` is monotonically increasing in `here_forage` and in hunger `(1 − energy)`.
- With nominal `dwell`, a starving elk (`energy ≈ 0`) on rich grass (`here_forage ≈ 1`) yields a `graze_value` that exceeds `stand_value()` (graze beats rest when hungry on forage).
- `graze_value ≥ stand_value()` always (eating is never worse than resting).

### 2. Produce six candidates in `herd_move` (`src/elk/movement.rs:204-421`)

After the four `Step` candidates are scored (keep that loop exactly as is), append
two more before the softmax:

- A `Stand` candidate: `Candidate { act: Act::Stand, score: stand_value(), penalty: 0.0, weight: 0.0 }`.
- A `Graze` candidate: `Candidate { act: Act::Graze, score: graze_value(grid.forage(elk.cell), elk.energy, params.dwell), penalty: 0.0, weight: 0.0 }` — reuse the `here_forage` already computed at `:298`.

Then:
- Include all candidates (up to six) in the `max`/softmax/weight computation. Replace the fixed `[_; 4]` arrays (`scores`, `cells`, `penalties`, `weights`) with a representation that holds six — a small `Vec` or `[_; 6]`. `Stand`/`Graze` have no destination cell (`cells[k] = None`).
- The softmax pick must dispatch on the chosen candidate's `Act`: `Act::Step(dx, dy)` → `elk.cell = next` (as today); `Act::Stand`/`Act::Graze` → leave `elk.cell` unchanged (it already equals `prev_cell` from `:230`). Set `chosen_act` accordingly.
- The hemmed-in branch still sets `chosen_act = Act::Stand`. Note: with a Stand candidate always present and finite (`score = 0`), `max` is now always finite, so the hemmed-in path effectively becomes "Stand wins" — keep the branch for the empty-options UI case but ensure `chosen_act = Act::Stand`.

### 3. Gate eating on `chosen_act == Graze` (`src/elk/metabolism.rs:33-76`)

- Add `&LastDecision` to the `graze` query: `Query<(&mut Elk, &LastDecision)>`.
- For each elk, compute `intake_this_tick` as today **only when** `last_decision.0.chosen_act == Act::Graze`; otherwise `intake_this_tick = 0.0` and the elk does not eat (no `eat_shrubs`/`grow_grass`, no energy gain). Set `elk.grazing = true` only on a tick where the elk chose `Graze` **and** actually took a worthwhile bite (or shrub); `false` otherwise.
- **Keep the bookkeeping for every elk:** the `intake_rate` EWMA update (`:70`) and the `sum`/`count` → `habitat_intake.mean` (`:71-75`) must run for all elk, grazers and movers alike, using each elk's `intake_this_tick` (0 for movers). This preserves the `forage_gate` signal.
- Import `Act` into `metabolism.rs`.

### 4. Tunables (`src/elk/components.rs`)

- Add `pub dwell: f32` to `ElkParams` (graze-appeal weight; the calibration knob between graze value and move scores). Document it: "weight on the graze candidate's value, in desire-magnitude units; higher = elk pause to feed more readily."
- In the defaults block (`:187-228`), set `dwell` to a value calibrated so a hungry elk on good grass reliably chooses `Graze` over stepping, while a fed elk drifts. Start around `dwell: 1.5` and adjust if the calibration test in step 1 needs it.
- Retune `bite` downward (e.g. `0.5` → `0.12`) so a committed graze depletes a patch over several ticks rather than one — turning the pause into a visible multi-tick dwell. Update the adjacent sustainability comment accordingly.

### 5. Mirror the six candidates in the harness (`src/sim_harness.rs:243-274`)

`FieldDecider::decide` must stay in lockstep so the probe matches real behavior.
Append `Stand` and `Graze` candidates (using `grid.forage(cell)` and `self.energy`),
include them in the deterministic best-pick, and set `chosen_act` from the winner.
Import `graze_value`/`stand_value` as needed.

## Files to Modify

- `src/elk/movement.rs` — `graze_value`/`stand_value` + tests; six-candidate assembly and dispatch in `herd_move`.
- `src/elk/metabolism.rs` — gate eating on `chosen_act == Act::Graze`; preserve intake bookkeeping for all elk.
- `src/elk/components.rs` — add `dwell` param; retune `bite`; update defaults/comments.
- `src/sim_harness.rs` — six candidates in `FieldDecider::decide`.

## Verification

```bash
just test-fast
```

New pure-function tests (`graze_value`/`stand_value` calibration and monotonicity)
must pass, and all existing tests must remain green. The library crate must compile
(`cargo test --lib` covers `sim_harness`, `ui`, `render`).

## Out of Scope

- Movement energy cost / `move_cost` ledger flow (future task).
- Re-expressing move scores in energy-equivalent units (future task; pragmatic currency for now).
- A macro-sim canary for the gate-feedback runaway (intake-bounded-ness under the new regime) — this is an integration check under `test-all`, run manually pre-merge, not part of this agent's gate. See Notes.

## Notes

- **Gate-feedback risk:** because eating now requires a deliberate stop, a mover posts 0-intake ticks, decaying its `intake_rate` EWMA, which closes `forage_gate` and amplifies migration — potentially a move→no-intake→more-move runaway. The unit tests pin the *scoring* intent; the *systemic* check (herd mean intake stays bounded, elk actually pause) belongs to a `test-all` macro canary and live `just run` observation. Flag for the reviewer to watch; if the herd over-migrates or never pauses, the lever is `dwell` (up) and `bite` (down).
- Calibration is the crux: `graze_value` must land in the same magnitude band as `desire·dir − penalty`. If `Graze` never (or always) wins in `just run`, adjust `dwell`.
- `Stand` being a finite `0`-score candidate means `max` is always finite — double-check no code path assumes `max` can be `-inf` except the genuinely empty-options UI case.

## Surface after this phase

- `graze_value(here_forage, energy, dwell) -> f32` and `stand_value() -> f32`, pure, exported from the `elk` module.
- `herd_move` produces up to six candidates (`Step×4`, `Stand`, `Graze`), softmaxes over all, and writes `chosen_act ∈ {Step, Stand, Graze}`. `Stand`/`Graze` leave `elk.cell` unchanged.
- `metabolism::graze` feeds an elk **iff** its `chosen_act == Act::Graze`; `elk.grazing` is true only on a chosen-and-fed tick. `intake_rate`/`HabitatIntake` bookkeeping still runs for every elk.
- `ElkParams` gains `dwell`; `bite` is retuned down.
- `FieldDecider::decide` mirrors the six-candidate decision.
- **Negative space:** `EnergyFlows`/`ledger.rs`, `forage_gate`, `combine_drives`, `Drives`, `grass_gradient` remain UNCHANGED; moves remain energy-free.
