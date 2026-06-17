# Agent Result: soil-type-riparian-steppe

date: 2026-06-17T09:28:51-04:00
session: completed
verification: passed
commits: 1
branch: chain-procgen-naturalism_claude_soil-type-riparian-steppe
surface deviations: none
session id: 72a8c38e-5be7-4856-adf6-10fa56877b93


## Summary

All verification passes. Implementation complete.

---

## Implementation summary

Added a `soil_type` riparian/steppe gradient field to the grid and wired it through world-gen and rendering:

1. **`src/grid.rs`** — Added `soil_type: Vec<f32>` field (init `0.0`), `soil_type(i)` getter, and `set_soil_type(i, v)` setter with clamp to `[0,1]`.

2. **`src/worldgen/soil_type.rs`** (new) — `seed_soil_type` derives riparian-ness from `water_prox` using a `[LO=0.2, HI=0.7]` ramp. Includes `riparian_hugs_water` unit test.

3. **`src/worldgen/mod.rs`** — Added `mod soil_type;` and inserted `soil_type::seed_soil_type` call between `seed_soil` and `seed_browse_cap`.

4. **`src/worldgen/vegetation.rs`** — Added `steppe = 1.0 - 0.5 * soil_type` factor multiplied into browse capacity for a gentle riparian suppression.

5. **`src/render.rs`** — `sync_tiles` now lerps between `steppe_dirt (0.80,0.72,0.52)` and `riparian_dirt (0.45,0.38,0.26)` by `soil_type` before the grass/water lerps.

All suites green: `--lib` (46 tests), `macro_sim` (3 tests), `balance` (1 test).

## Notes

None.

## Surface Deviations

None.

## Commits

```
1741022 feat: soil-type riparian/steppe field with two-tone dirt render
```

## Build & Test Output (last 30 lines)

```

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 39.07s

warning: unused import: `Heading`
  --> src/bin/../river/mod.rs:14:16
   |
14 | pub use spec::{Heading, RiverSpec};
   |                ^^^^^^^
   |
   = note: `#[warn(unused_imports)]` on by default

warning: method `soil` is never used
   --> src/bin/../grid.rs:153:12
    |
75  | impl Grid {
    | --------- method in this implementation
...
153 |     pub fn soil(&self, index: usize) -> f32 {
    |            ^^^^
    |
    = note: `#[warn(dead_code)]` on by default

warning: `mesopotamia` (bin "demo1") generated 2 warnings (run `cargo fix --bin "demo1"` to apply 1 suggestion)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.20s
     Running tests/balance.rs (/home/saxon/.cache/mesopotamia-shared-target/debug/deps/balance-c6526f02b45cbf9e)

running 1 test
test default_params_inside_balanced_envelope ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.90s
```
