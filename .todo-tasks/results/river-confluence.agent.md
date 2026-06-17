# Agent Result: river-confluence

date: 2026-06-16T23:44:11-04:00
session: completed
verification: passed
commits: 1
branch: chain-procgen-rest_claude_river-confluence
surface deviations: none
session id: 6fe27fd6-c710-4f2b-b1ec-6e5fbb20a6cd


## Summary

Implementation complete. Commit `b899d4d` is on the branch.

---

## Summary

Added river confluence to the watershed generator in two files:

**`src/river/spec.rs`** — Added `confluence_pairs: Vec<(usize, usize)>` to `RiverSpec` with default `vec![(1, 0)]` (river 1 merges into river 0).

**`src/river/mod.rs`** — In `generate_river_inner`'s main carve loop, when river `i` is declared a child:
- Filters the parent's already-carved centerline to cells in the lower 40% of the map (row > 60% height)
- Samples one as the carve goal using the per-river seeded `rrng` (deterministic)
- Falls back to the full parent centerline if no lower cells exist
- Carves and rasterizes at full `spec.radius`/`spec.core` as a main channel
- Child is still pushed to `mains` so fords, tributaries, and water prox remain unaffected

Tests: Updated `rivers_flow_top_to_bottom` to skip the bottom-edge assertion for children and check parent centerline membership instead. Added `confluence_child_joins_parent` pinning both the centerline membership and the lower-portion placement. All 47 lib tests and all 3 macro_sim tests pass (including `herds_reach_the_far_edge`).

## Notes

The `Heading` import produces an unused-import warning in the binary target — this pre-existed and is unrelated to this change.

## Surface Deviations

None.

## Commits

```
b899d4d feat: river confluence — child main merges into parent centerline
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
   --> src/bin/../grid.rs:140:12
    |
73  | impl Grid {
    | --------- method in this implementation
...
140 |     pub fn soil(&self, index: usize) -> f32 {
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

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 38.78s
```
