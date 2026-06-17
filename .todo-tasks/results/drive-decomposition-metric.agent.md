# Agent Result: drive-decomposition-metric

date: 2026-06-16T20:54:53-04:00
session: completed
verification: passed
commits: 1
branch: milestone1_claude_drive-decomposition-metric
surface deviations: declared
session id: 61096735-08d4-4d03-b919-353e6f121144


## Summary

Commit confirmed. Here's the implementation summary.

---

The plan was implemented in full across four files:

- **`src/elk/movement.rs`**: Added `migration_residual(natural_strength, quiet) -> f32`, `pub struct Drives { sep, coh, grass, social, migration }` with `total()` and `migration_share()`, `pub fn combine_drives(...)  -> Drives`. Refactored `herd_move` to call `combine_drives` and steer by `drives.total()`. Added 9 tests (4 for `migration_residual`, 5 for `Drives`/`combine_drives`) with concrete and metamorphic coverage.
- **`src/elk/components.rs`**: Added `pub quiet: f32` to `ElkParams` with default `quiet = 2.0` and `migration = 0.7`.
- **`src/ui.rs`**: Added `slider(ui, &mut p.quiet, 0.05..=3.0, "migration crossover (quiet)")` after the migration slider.
- **`src/elk/mod.rs`**: Extended the `pub use` to include `Drives`, `combine_drives`, `migration_residual`.

All verification passes: 37 lib unit tests, 3 macro soak invariants, and both binary builds.

## Notes

- **Default adjustment**: The plan suggested `quiet = 0.5` as a starting point. At typical natural_strength ≈ 2.0 (active, well-fed herd), `migration_residual(2.0, 0.5) = 0.2` — a 5× reduction in migration force that empirically risks failing the `herds_reach_the_far_edge` soak test. Chose `quiet = 2.0, migration = 0.7` instead: at natural_strength = 2.0, effective pull = `0.7 × 0.5 = 0.35`, matching the old constant exactly. At natural_strength = 0 (isolated elk), effective pull = 0.7 — stronger than before, reinforcing the "fallback" intent. The `balancing-param-sweep` task was already named as the proper tuning venue.
- `migration_share` carries `#[allow(dead_code)]` following the `cross_desire` pattern — it's a published metric that downstream tasks (`ui-declarative-panels-graphs`, `balancing-param-sweep`) will consume.

## Surface Deviations

None. All declared surface items are present with the exact signatures specified: `Drives { sep, coh, grass, social, migration: Vec2 }`, `total() -> Vec2`, `migration_share() -> f32` (sum-of-magnitudes denominator, 0 when idle), `combine_drives(sep, coh, grass_dir, social, &ElkParams, pressure, energy) -> Drives`, `migration_residual(natural_strength, quiet) -> f32`, and `ElkParams.quiet: f32` with a UI slider in `behaviour_tab`.

## Commits

```
4073d54 feat: drive decomposition metric + residual migration (combine_drives)
```

## Build & Test Output (last 30 lines)

```

running 37 tests
.....................................
test result: ok. 37 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s


running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s


running 3 tests
...
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 13.30s


running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

unit tests: PASS

[1m=== build (cargo build) ===[0m
build demo1: PASS
build mesopotamia: PASS

[1m=== runtime launch ===[0m
skipped (--no-run)

[1;32mALL SMOKE CHECKS PASSED[0m
```
