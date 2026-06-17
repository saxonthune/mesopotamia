# Agent Result: regional-fertility

date: 2026-06-16T23:38:58-04:00
session: completed
verification: passed
commits: 1
branch: chain-procgen-rest_claude_regional-fertility
surface deviations: none
session id: 373f03ae-844e-4736-81d4-67a30d2f66f3


## Summary

Commit verified. Implementation is complete.

---

## Implementation summary

Added a coarse macro-octave to the soil field in `src/grid.rs`:

1. **Constants** — `MACRO_SEED: u64 = 0xA7C3`, `MACRO_PASSES: usize = 14`, `MACRO_FLOOR: f32 = 0.4` added near the existing soil constants.

2. **`seed_soil` blend** — generates a second noise field from a fresh `StdRng` seeded with `MACRO_SEED`, normalizes it, then multiplies the two factors:
   ```rust
   let fine   = SOIL_FLOOR + (1.0 - SOIL_FLOOR) * p;
   let region = MACRO_FLOOR + (1.0 - MACRO_FLOOR) * m;
   grid.set_soil(index, fine * region);
   ```
   `soil` and `capacity` stay in [0, 1] / [0, MAX_GRASS].

3. **`soil()` accessor** — public read accessor added to `Grid` so tests (and the next chain phase) can inspect values.

4. **`soil_has_regional_spread` test** — verifies cell-level min/max spread > 0.3 and values stay in [0, 1]. Uses fixed seeds → deterministic.

5. **`demo1.rs` path fix** — pre-existing broken `#[path = "../river.rs"]` → `#[path = "../river/mod.rs"]` unblocked `cargo test --test macro_sim`.

All 46 lib tests and all 3 macro_sim integration tests pass.

---

## Notes

- The original test approach (compare column-third means) failed because with 14 smoothing passes the dominant macro feature can run vertically, making left/right column averages nearly identical. Switched to a cell-level min/max spread check, which the plan explicitly offers as an alternative ("OR simply that `min < threshold < max` with substantial range").
- The `demo1.rs` fix was not in the plan scope but was necessary to run the required `cargo test --test macro_sim` verification. The change is a pure path correction, not a logic change to river generation.
- The `soil()` accessor generates a `dead_code` warning in the demo1 binary compilation because `#[cfg(test)]` code isn't compiled into binaries. It will be used in the next chain phase (river-confluence reads soil), so suppressing it with `#[allow(dead_code)]` was skipped.

## Surface Deviations

None.

## Commits

```
4a9fe43 feat: regional fertility — macro-octave soil blend
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

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 38.79s
```
