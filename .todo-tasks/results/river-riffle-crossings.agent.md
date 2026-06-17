# Agent Result: river-riffle-crossings

date: 2026-06-17T09:24:09-04:00
session: completed
verification: passed
commits: 1
branch: chain-procgen-naturalism_claude_river-riffle-crossings
surface deviations: declared
session id: 35cd9001-12a2-4553-840a-a02a7ca46355


## Summary

Implementation complete. Here's a summary of what was done:

**`src/river/spec.rs`** — Removed `radius`, `ford_spacing`, `ford_depth`. Added `riffle_depth` (0.3), `riffle_radius` (4), `pool_radius` (2), `riffle_ford_threshold` (0.45), `riffle_passes` (6) with matching `Default` values.

**`src/river/raster.rs`** — Deleted `ford_indices`, `stamp_fords`, and their 3 unit tests. Added `stamp_main_channel` (lerps depth and radius from a per-position noise profile, calls `rasterize` per cell) and `tag_shallows_as_fords` (sets ford flag on any cell where `0 < water <= threshold`). Imports `lerp` and `RiverSpec` from sibling modules.

**`src/river/mod.rs`** — Updated imports (removed `rasterize`/`stamp_fords`, added `stamp_main_channel`/`tag_shallows_as_fords`, added `use crate::field`). In the main loop, replaced the single uniform `rasterize` call with a per-river `value_noise` + `normalize` profile fed into `stamp_main_channel`. Replaced `stamp_fords` call with `tag_shallows_as_fords`. Updated `fords_carry_low_water` to assert `w ∈ (0, riffle_ford_threshold]`. Added `river_has_riffles_and_pools` test.

All 45 lib tests and all 3 macro_sim tests (including `herds_reach_the_far_edge`) pass.

## Notes

None.

## Surface Deviations

None. All declared Surface items match exactly: `radius`/`ford_spacing`/`ford_depth` removed; five riffle knobs added with specified defaults; `stamp_fords`/`ford_indices` gone; `stamp_main_channel`/`tag_shallows_as_fords` present as `pub(super)`; `generate_water` signature/return unchanged; `src/elk/movement.rs`, `src/grid.rs`, `src/worldgen/`, and `src/render.rs` untouched.

## Commits

```
400cac8 feat: river riffle-crossings — variable depth/width mains + shallow fords
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

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 38.50s
```
