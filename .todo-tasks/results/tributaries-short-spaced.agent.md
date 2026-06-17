# Agent Result: tributaries-short-spaced

date: 2026-06-17T09:19:16-04:00
session: completed
verification: passed
commits: 1
branch: chain-procgen-naturalism_claude_tributaries-short-spaced
surface deviations: none
session id: 50b37099-bd67-4fc8-a182-1c943674beb7


## Summary

All 50 tests pass (47 lib + 3 macro_sim), one commit made.

---

Replaced the old edge-sourced tributary algorithm with short, spaced feeders that branch laterally off each main. The three modified files match the Surface contract exactly.

## Notes

- The `trib_spacing.max(1)` guard in the `step_by` call prevents a panic if someone sets `trib_spacing = 0`; the plan didn't mention it but it's the only way to avoid a `step_by(0)` panic without changing the API.
- The source-cell skip (`grid.water(source) > 0.0`) means that if a proposed source already sits inside a channel (e.g. where mains are close together), that feeder is silently dropped. This is correct per the plan and keeps the code clean.
- The macro_sim tests take ~40 s at the first run because the shared target dir needs a full link step; subsequent runs are fast.
- Two pre-existing warnings appeared (`unused import: Heading`, `dead_code: Grid::soil`) — both are from trunk and untouched by this phase.

## Surface Deviations

None.

## Commits

```
108647e feat(river): short spaced tributaries along each main channel
```

## Build & Test Output (last 30 lines)

```

warning: unused import: `Heading`
  --> src/bin/../river/mod.rs:14:16
   |
14 | pub use spec::{Heading, RiverSpec};
   |                ^^^^^^^
   |
   = note: `#[warn(unused_imports)]` on by default

warning: method `soil` is never used
   --> src/bin/../grid.rs:148:12
    |
71  | impl Grid {
    | --------- method in this implementation
...
148 |     pub fn soil(&self, index: usize) -> f32 {
    |            ^^^^
    |
    = note: `#[warn(dead_code)]` on by default

warning: `mesopotamia` (bin "demo1") generated 2 warnings (run `cargo fix --bin "demo1"` to apply 1 suggestion)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.20s
     Running tests/macro_sim.rs (/home/saxon/.cache/mesopotamia-shared-target/debug/deps/macro_sim-7edbb8e29410321d)

running 3 tests
test population_does_not_collapse_or_explode ... ok
test grass_never_fully_collapses ... ok
test herds_reach_the_far_edge ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 38.72s
```
