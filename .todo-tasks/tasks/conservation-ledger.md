# Conservation Ledger

## Motivation

Rung 5 (doc02.03): the most unambiguous class of check. An emergent system has quantities that must
reconcile, and a violated reconciliation is a bug with no "is this intended?" ambiguity — unlike
"elk seem stuck." The cohort counters already hold every term of the population identity (doc02.02,
and `sim_harness::run_metrics` already sums `alive + deaths + departures`); energy has a closeable
ledger. Today neither is asserted as an invariant.

## Do NOT

- Do NOT change simulation behaviour or balance. This phase adds pure checks and tests only.
- Do NOT silently widen a tolerance to make the energy ledger pass — if the books do not close,
  report the leak in the result notes and pin the *actual* behaviour with a test that documents it.
- Do NOT add UI (this rung is a harness-side guarantee, not an on-screen panel).

## Plan

### 1. Population identity (pure, `src/elk/components.rs` or `src/elk/metabolism.rs`)

Add a pure function over the cohorts:

```rust
pub fn population_balances(herds: &Herds) -> bool // Σ(alive+deaths+departures) accounted; spawned == that
```

Express it precisely against `Cohort` (`alive`, `deaths`, `departures`, `peak`). Unit-test the
identity on hand-built `Cohort`s.

### 2. Energy ledger (pure)

Name each source/sink from the metabolism/graze code: intake (`graze`: `graze_yield`·bite and
`shrub_energy`), drain (`energy_drain`/tick), swim cost (`swim_drain`). Add a pure function that,
given per-tick totals (intake, drain, swim, births' starting energy, deaths' lost energy), returns
the expected net change in total energy, plus a checker `energy_ledger_closes(before, after, flows,
tol) -> bool`. Unit-test that a hand-constructed balanced tick closes and an unbalanced one fails.

### 3. Harness invariant assertions (`src/sim_harness.rs` + an integration test)

In a `test-all` integration test (the macro-sim harness), run a headless sim for N ticks and assert
`population_balances` holds at run end, and that the energy ledger closes each tick within a stated
tolerance (accumulate the per-tick flows; compare to the measured total-energy delta). Wire helpers
into `sim_harness.rs` so the test reads totals without duplicating queries.

## Files to Modify

- `src/elk/components.rs` (or `metabolism.rs`) — `population_balances`, energy-ledger pure fns + unit tests.
- `src/sim_harness.rs` — total-energy / flow accessors for the harness.
- `tests/` integration (or the existing macro-sim test binary) — the run-level invariant assertions.

## Verification

```bash
just test-fast
just test-all
```

## Out of Scope

- UI for the ledger.
- Decision record, overlays, histograms, events, probes (other phases).

## Notes

- Chain `sim-observability`, phase 5 of 6. Mostly independent — components/harness side; safe to
  triage against current code (`sim_harness.rs` shown; `Cohort` in `components.rs`).
- The energy ledger may surface a real leak (e.g. digestion/`shrub_energy` bookkeeping, or energy
  lost when an elk despawns). That is a finding, not a failure to hide — report it.
- Pairs naturally with phase 6 (probes share the harness).
- Patterns: doc02.01, doc02.02 (population identity at `run_metrics`; foraging economics), verification skill.

## Surface after this phase

- `pub fn population_balances(herds: &Herds) -> bool` and pure energy-ledger functions
  (`energy_ledger_closes(...)` + a per-tick expected-delta fn), exported from `crate::elk`.
- Harness accessors in `sim_harness.rs` for total elk energy and per-tick flow totals.
- An integration test asserting both invariants over a headless run; simulation behaviour unchanged.
