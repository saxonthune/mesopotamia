# Agent Result: regenerate-control

date: 2026-06-17T12:13:09-04:00
session: completed
verification: passed
commits: 1
branch: chain-world-regenerate_claude_regenerate-control
surface deviations: none
session id: f9d04bf4-52a3-49b0-8329-5ef2a33ba56e


## Summary

All verification commands pass. Here is the implementation summary:

---

**What was implemented:**

1. **`src/elk/spawn.rs`** — Added `teardown` system that iterates all `Elk` entities and despawns them via `Commands`, then resets `Spawner`, `Herds`, `Packs`, and `DriveSamples` to their defaults. `DriveSample` and `DriveSamples` were added to the module's import list.

2. **`src/elk/mod.rs`** — Registered `teardown` on `OnExit(Sim::Running)` so it fires on every regenerate but never at boot.

3. **`src/ui.rs`** — Added `world_seed: ResMut<WorldSeed>` and `next_state: ResMut<NextState<Sim>>` params to `control_panel`, and placed the `⟳ regenerate` button in the existing tab-bar `horizontal` row after `speed_inline`. Button draws a fresh `rand::random()` seed and transitions `Sim` back to `Generating`.

4. **`src/sim_harness.rs`** — Added `spawner_elapsed(world: &World) -> u32` helper to expose the `pub(crate)` `Spawner.elapsed` field to integration tests.

5. **`tests/regenerate.rs`** — New integration test that runs until elk appear, captures the pre-regenerate water signature, injects a deterministic seed (`0xDEAD_BEEF_CAFE_1234`), triggers a state transition, runs one update, then asserts elk count = 0, `Spawner.elapsed` = 0, `Herds.cohorts` empty, and grid water layout changed.

6. **`.rhidoc/03-milestones/01-grazers/02-structure.md`** — Added `## Sim State` subsection and updated `## Schedules` with a note about state-gating.

## Notes

- The `sim_harness::spawner_elapsed` helper was added because `Spawner.elapsed` is `pub(crate)` — inaccessible from integration tests directly. The helper lives inside the crate, bridging the visibility boundary cleanly without making the field fully public.
- The regenerate test completes very fast (0.35s) because the teardown fires before `spawn_waves` gets a chance to run, leaving elk count verifiably at 0 after just one `app.update()`.
- The doc `## Schedules` sentence was extended minimally to mention state-gating, as the plan suggested.

## Surface Deviations

None.

## Commits

```
95990d4 feat: regenerate-control — teardown, UI button, test, doc
```

## Build & Test Output (last 30 lines)

```
test river::tests::generation_is_deterministic ... ok
test worldgen::soil::tests::soil_has_regional_spread ... ok

test result: ok. 52 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.11s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.20s
     Running tests/macro_sim.rs (target/debug/deps/macro_sim-7edbb8e29410321d)

running 3 tests
test population_does_not_collapse_or_explode ... ok
test grass_never_fully_collapses ... ok
test herds_reach_the_far_edge ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 35.36s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.21s
     Running tests/balance.rs (target/debug/deps/balance-c6526f02b45cbf9e)

running 1 test
test default_params_inside_balanced_envelope ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.63s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.22s
     Running tests/regenerate.rs (target/debug/deps/regenerate-652770ef9a59e3cc)

running 1 test
test regenerate_teardown_and_rebuild ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.39s
```
