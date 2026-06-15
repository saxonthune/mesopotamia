# Agent Result: macro-state-test-harness

date: 2026-06-15T00:26:59-04:00
session: completed
verification: passed
commits: 1
branch: chain-harness-and-reorg_claude_macro-state-test-harness
surface deviations: declared
session id: c7e374cb-796f-4500-897c-edf787c0196d


## Summary

All verification passes. Here's the summary:

## Implementation Summary

**What was built:**

1. **`src/lib.rs`** — New library target with `pub mod` re-exports of all six simulation modules (`field`, `grid`, `elk`, `render`, `ui`, `river`). Cargo auto-detects it as the `mesopotamia` crate, no `Cargo.toml` changes needed.

2. **`src/main.rs`** — Removed the local `mod` declarations and `use crate::` imports; replaced with `use mesopotamia::...` imports from the lib. The `main()` function body is unchanged. `demo1.rs` is untouched.

3. **`tests/macro_sim.rs`** — Headless soak harness with:
   - `make_app()` — builds `MinimalPlugins + ScheduleRunnerPlugin::run_once() + ManualDuration(100ms) + (GridPlugin, ElkPlugin, RiverPlugin)` 
   - `headless(ticks) -> App` — helper declared per the Surface contract (kept with `#[allow(dead_code)]`)
   - Three metric helpers: `elk_count`, `total_grass`, `max_col_reached`
   - Three invariant tests, all passing in ~10 seconds over 3000 ticks

**Observed baseline (3000 ticks):**
- Population: 60–180 elk (ceiling 1600; floor > 0)
- Total grass min: 324.6 (floor 50.0, ~6× margin)
- Max col reached: 255 (EDGE_COL = 254; floor 244)

## Notes

- Bevy 0.18 does **not** fire `FixedUpdate` on the very first `app.update()` call (frame 0). Grass is legitimately 0 before any growth tick runs, so `grass_never_fully_collapses` skips a 5-tick warmup before tracking the minimum. The comment in the test explains why.
- `headless()` is unused by the current tests (all three track metrics over time, not just at the end). It's kept because the Surface declares it and downstream tasks will import it.
- The three tests run in ~10 seconds serially (each 3000 ticks). They run concurrently by default in `cargo test`, keeping wall-clock time reasonable.
- `sync_elk_transform` and `sync_elk_color` run under `MinimalPlugins` as harmless no-ops on the elk's `Sprite`/`Transform` components — elk have these components and the queries execute, they just update components nobody renders.

## Surface Deviations

None. All declared Surface elements are present:
- `src/lib.rs` exists, exposing `pub mod {field, grid, elk, render, ui, river}`
- `tests/macro_sim.rs` exists with `headless(ticks: u32) -> App` and three passing invariant tests
- `src/main.rs` consumes the lib crate, no local module re-declarations
- `ElkPlugin` is still a single bundled plugin; `demo1.rs` still uses `#[path]` includes and is unchanged

## Commits

```
1f5133f feat: add lib target and headless macro-state test harness
```

## Build & Test Output (last 30 lines)

```

running 27 tests
...........................
test result: ok. 27 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s


running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s


running 3 tests
...
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 9.96s


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
