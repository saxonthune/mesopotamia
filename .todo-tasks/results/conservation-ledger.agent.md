# Agent Result: conservation-ledger

date: 2026-06-17T15:35:26-04:00
session: completed
verification: passed
commits: 2
branch: chain-sim-observability_claude_conservation-ledger
surface deviations: declared
session id: 93449e83-45a7-42b0-9492-7eebaf6c2358


## Summary

All tests pass and 2 commits are made. Here's a summary of what was implemented.

---

**Implementation Summary**

All three plan steps are implemented and verified:

**Step 1 — Population identity:**
- Added `spawned: u32` to `Cohort` (set to wave size in `spawn_waves`)
- `population_balances(herds: &Herds) -> bool` in `src/elk/ledger.rs` checks `alive + deaths + departures == spawned` for every cohort
- Unit-tested with hand-built cohorts (balanced/unbalanced cases)

**Step 2 — Energy ledger:**
- `EnergyFlows` resource accumulates per-tick: intake (post-cap), drain, swim (post-clamp), births, deaths_energy (dying-elk overshoot), departures_energy (energy exiting with departing elk)
- `energy_expected_delta(flows) -> f32` and `energy_ledger_closes(before, after, flows, tol) -> bool` — pure functions
- Unit-tested with hand-constructed tick scenarios including the deaths_energy residual edge case
- Systems modified (observationally, no behavior change): `graze`, `metabolize`, `herd_move`, `spawn_waves`, `cull`

**Step 3 — Harness + integration test:**
- `total_elk_energy(world: &World) -> f32` and `elk_flows(world: &World) -> EnergyFlows` accessors in `sim_harness.rs`
- `tests/conservation_ledger.rs` with two tests:
  - `population_identity_holds_after_run` — checks cohort balance after 300 ticks ✓
  - `energy_ledger_closes_each_tick` — checks ledger to 0.01 tolerance per tick ✓

## Notes

- The `departures_energy` flow term was added beyond the plan's named list (intake, drain, swim, births, deaths' lost energy) because departing elk carry their energy out of the ledger — without it the books wouldn't close. This is arguably what "name each source/sink" implicitly requires.
- Energy capping at 1.0 is handled correctly by tracking actual intake (`new_energy - old_energy`), not raw intake.
- Swim clamping at 0.0 is tracked as `before - after` so the actual cost is captured.
- The `deaths_energy` term correctly accounts for the below-zero overshoot (elk die at energy ≤ 0, not exactly 0) — verified with a dedicated unit test.
- The ledger closes within floating-point error (well under the 0.01/tick tolerance). No genuine leak was found.
- Simulation behavior is unchanged — all three macro_sim invariants still pass.

## Surface Deviations

- `energy_ledger_closes(...)` has the parameter order `(before, after, flows, tol)` which matches the plan's description exactly.
- `EnergyFlows` has 6 fields rather than the 5 named in the plan (adds `departures_energy`) — necessary to close the ledger. The function signatures otherwise match.
- All other Surface symbols are present with the declared signatures.

## Commits

```
c1e387a fix: suppress dead_code warnings on ledger exports (used only in tests)
942e121 feat: conservation ledger — population identity + energy balance checks
```

## Build & Test Output (last 30 lines)

```

     Running tests/conservation_ledger.rs (target/debug/deps/conservation_ledger-fba9c4fd95c1473c)

running 2 tests
test population_identity_holds_after_run ... ok
test energy_ledger_closes_each_tick ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.61s

     Running tests/macro_sim.rs (target/debug/deps/macro_sim-7edbb8e29410321d)

running 3 tests
test population_does_not_collapse_or_explode ... ok
test grass_never_fully_collapses ... ok
test herds_reach_the_far_edge ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 43.88s

     Running tests/regenerate.rs (target/debug/deps/regenerate-652770ef9a59e3cc)

running 1 test
test regenerate_teardown_and_rebuild ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.49s

   Doc-tests mesopotamia

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```
