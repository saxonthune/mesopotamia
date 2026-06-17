# Agent Result: sim-state-machine

date: 2026-06-17T12:01:29-04:00
session: completed
verification: passed
commits: 1
branch: chain-world-regenerate_claude_sim-state-machine
surface deviations: none
session id: b91202ca-0a96-46e9-8c05-f7ec17fe82ab


## Summary

All verification passed. Implementation is complete.

---

The `Sim` state machine is in place. Here's what was done:

- **`src/sim.rs`** (new): `Sim` enum (`Generating` default, `Running`) + `SimStatePlugin` calling `init_state::<Sim>()`.
- **`src/lib.rs`**: added `pub mod sim;`.
- **`src/main.rs`** and **`src/bin/demo1.rs`**: imported and registered `SimStatePlugin`.
- **`src/worldgen/mod.rs`**: `generate_world` now runs on `OnEnter(Sim::Generating)`, calls `grid.reset()` first, and ends with `next.set(Sim::Running)`.
- **`src/grid.rs`**: added `Grid::reset()` (replaces self with `Grid::new` at same dimensions), unit-tested; gated `GridPlugin` FixedUpdate systems on `in_state(Sim::Running)`.
- **`src/elk/mod.rs`**: gated all FixedUpdate systems on `in_state(Sim::Running)`; `tally_herds` left ungated.
- **`src/sim_harness.rs`**: added `StatesPlugin` and `SimStatePlugin` to `make_app`.

## Notes

The plan correctly predicted the `StatesPlugin` issue: `MinimalPlugins` does not include it, so `init_state` panics without it being added explicitly. Added `app.add_plugins(StatesPlugin)` before `SimStatePlugin` in `make_app`. One-update startup offset (worldgen + transition on first update) was within tolerance for all macro tests.

## Surface Deviations

None.

## Commits

```
f284671 feat: introduce Sim state machine (Generating → Running)
```

## Build & Test Output (last 30 lines)

```
test river::tests::per_river_params_differ ... ok
test river::tests::confluence_child_joins_parent ... ok
test river::tests::tributaries_join_a_main_channel ... ok
test river::tests::main_channel_has_deep_water ... ok
test river::tests::water_levels_span_expected_range ... ok
test river::tests::rivers_flow_top_to_bottom ... ok
test river::tests::fords_carry_low_water ... ok
test river::tests::lakes_are_seeded ... ok
test worldgen::soil::tests::soil_has_regional_spread ... ok
test river::tests::generation_is_deterministic ... ok

test result: ok. 52 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.11s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.23s
     Running tests/macro_sim.rs (target/debug/deps/macro_sim-7edbb8e29410321d)

running 3 tests
test population_does_not_collapse_or_explode ... ok
test grass_never_fully_collapses ... ok
test herds_reach_the_far_edge ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 35.88s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.20s
     Running tests/balance.rs (target/debug/deps/balance-c6526f02b45cbf9e)

running 1 test
test default_params_inside_balanced_envelope ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.85s
```
