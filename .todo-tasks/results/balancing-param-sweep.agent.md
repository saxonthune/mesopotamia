# Agent Result: balancing-param-sweep

date: 2026-06-16T21:48:55-04:00
session: completed
verification: passed
commits: 1
branch: chain-demo-readouts_claude_balancing-param-sweep
surface deviations: declared
session id: 918146e7-7668-4def-b5cd-b0cf17282a8a


## Summary

Commit landed. All verification commands pass.

---

## Implementation summary

Implemented the full balancing parameter sweep plan in five file changes:

1. **`src/elk/components.rs`** — added `#[derive(Clone)]` to `ElkParams` so candidates can be cloned for the migration=0 counterfactual.

2. **`src/sim_harness.rs`** (new) — the shared headless harness: `make_app()`, `elk_count()`, `max_col_reached()` (lifted from `tests/macro_sim.rs`), plus `RunMetrics`, `run_metrics(params, ticks)`, and `journey_natural(params, ticks)` implementing the doc02.02 objective.

3. **`src/lib.rs`** — added `pub mod sim_harness;`.

4. **`tests/macro_sim.rs`** — removed the now-duplicated private helpers, imports them from `mesopotamia::sim_harness` instead. All three existing tests still pass.

5. **`src/bin/sweep.rs`** (new) — coarse grid over `migration` (4 values), `quiet` (4 values), `energy_drain` (3 values) = 48 candidates. For each: computes `run_metrics` and `journey_natural`, then prints rows ranked by balance score. Reports a summary count.

6. **`tests/balance.rs`** (new) — single fast test (300 ticks) asserting default params satisfy `survival ≥ 0.1`, `journey_natural ≥ 10`, `mean_migration_share ≤ 0.99` — loose per doc02.02.

## Notes

The sweep reports 0/48 balanced candidates at its current thresholds (`EDGE_BAND=200`, 300 ticks). This is correct and expected: at 300 ticks with `migration=0`, elk reach only col ~20–28 on natural drives. The sweep's note-on-empty message tells the operator to widen the grid or increase ticks. The balance *test* uses a much looser `EDGE_BAND=10` so it pins the mechanics (elk move on natural drives) not the full doc02.02 journey claim, which requires a longer run to validate.

The `render` and `ui` modules are in `lib.rs` and thus compile into the library, which is fine since `sim_harness` doesn't pull in any rendering — Bevy's `MinimalPlugins` excludes rendering, so headless builds stay clean.

## Surface Deviations

None. All declared Surface symbols are present with the specified signatures:
- `mesopotamia::sim_harness::make_app()` ✓
- `mesopotamia::sim_harness::run_metrics(ElkParams, ticks) -> RunMetrics` ✓
- `mesopotamia::sim_harness::journey_natural(ElkParams, ticks) -> usize` ✓
- `mesopotamia::sim_harness::elk_count`, `max_col_reached` (shared metric readers) ✓
- `RunMetrics { survival: f32, max_col: usize, mean_migration_share: f32 }` ✓
- `src/bin/sweep.rs` runnable, prints ranked balanced candidates ✓
- Fast regression test in `tests/balance.rs` ✓
- No UI consumption; thresholds deliberately loose ✓

## Commits

```
48ddaf6 feat: balancing parameter sweep harness
```

## Build & Test Output (last 30 lines)

```
0.500      1.000   0.0040        1.000    24       0.014     no
1.500      0.300   0.0060        1.000    24       0.015     no
0.500      3.000   0.0040        1.000    22       0.015     no
1.500      0.300   0.0020        1.000    21       0.018     no
0.500      2.000   0.0020        1.000    25       0.018     no
0.900      2.000   0.0060        1.000    22       0.022     no
0.500      2.000   0.0040        1.000    23       0.023     no
0.500      2.000   0.0060        1.000    20       0.023     no
1.500      1.000   0.0020        1.000    21       0.024     no
0.900      1.000   0.0060        1.000    21       0.026     no
0.900      2.000   0.0040        1.000    20       0.026     no
0.900      1.000   0.0020        1.000    22       0.027     no
0.900      3.000   0.0040        1.000    21       0.028     no
0.500      3.000   0.0060        1.000    22       0.029     no
0.500      3.000   0.0020        1.000    21       0.031     no
0.900      1.000   0.0040        1.000    24       0.031     no
0.900      3.000   0.0060        1.000    23       0.032     no
1.500      1.000   0.0040        1.000    26       0.038     no
0.900      2.000   0.0020        1.000    23       0.043     no
1.500      1.000   0.0060        1.000    22       0.043     no
1.500      2.000   0.0040        1.000    22       0.044     no
0.900      3.000   0.0020        1.000    21       0.044     no
1.500      2.000   0.0060        1.000    21       0.050     no
1.500      3.000   0.0040        1.000    23       0.061     no
1.500      3.000   0.0020        1.000    24       0.065     no
1.500      2.000   0.0020        1.000    22       0.069     no
1.500      3.000   0.0060        1.000    21       0.074     no

0/48 candidates are balanced.
No balanced candidates found at these thresholds — widen the grid or loosen thresholds.
```
