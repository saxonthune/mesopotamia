# Balancing parameter sweep (find the balanced set)

## Motivation

To balance the demo around a real answer (doc03.01.01) we need a headless tool that scores an
`ElkParams` against the balance objective (doc02.02): **survival**, the **counterfactual
journey** (herd reaches the far edge with migration off), and **mean migration_share** over the
run. Sweeping params over the slider ranges then finds the sets that clear all three, and pinning
the winning envelope makes it a regression net. Builds on the existing headless harness
(`tests/macro_sim.rs`) and the verification skill's "seeded soak run" rung.

Read doc02.02 (the equations this implements) and doc02.01 before starting.

## Do NOT

- **Do NOT** put the long-running sweep in the default `cargo test` path. The sweep is a search;
  gate it behind `src/bin/sweep.rs` so `cargo test` stays fast.
- **Do NOT** recompute drive shares from scratch in the test — read `DriveSamples` (from the
  `drive-sampling-resource` Surface). Average its per-slot `migration_share()` over ticks.
- **Do NOT** assert bit-determinism. Fixed-seed reproducibility is the goal (verification skill).
- **Do NOT** tighten or weaken the existing `tests/macro_sim.rs` invariant envelopes.
- **Do NOT** deviate from doc02.02's metric definitions — survival, journey, share are specced
  there; implement them, don't reinvent them.

## Plan

### 1. Make the headless harness reusable

`tests/macro_sim.rs` has `make_app`/`headless` but they are test-private. Lift the shared pieces
(`make_app`, the metric helpers `elk_count`, `max_col_reached`, and a `total_deaths`/`spawned`
reader over `Herds`) into a small `pub` module the bin and the test can both use — prefer
`src/sim_harness.rs` re-exported by `lib.rs` (one concept: headless stepping + readers, per
doc02.01). Keep `tests/macro_sim.rs` working by importing from there.

### 2. RunMetrics reducer (`src/sim_harness.rs`)

```rust
pub struct RunMetrics {
    pub survival: f32,           // 1 - deaths/spawned (doc02.02)
    pub max_col: usize,          // farthest column reached
    pub mean_migration_share: f32, // time-averaged over DriveSamples per-slot
}
```

`run_metrics(params: ElkParams, ticks: u32) -> RunMetrics` — build the app, `insert_resource`
the given `ElkParams` (overriding the plugin default), step `ticks`, sampling
`DriveSamples` each tick to accumulate `mean_migration_share`, and read survival from `Herds`
(`deaths`, `departures`, `alive` → `spawned`).

### 3. Counterfactual journey

`journey_natural(params, ticks) -> usize` runs `run_metrics` with a clone of `params` whose
`migration = 0.0` and returns its `max_col`. Balanced requires this to reach the edge band
(doc02.02) — i.e. naturals carry the journey unaided.

### 4. The sweep (`src/bin/sweep.rs`)

Iterate `ElkParams` candidates over the `behaviour_tab` slider ranges (coarse grid or seeded
random search across the weights, `quiet`, and metabolism). For each, compute
`survival`, `mean_migration_share`, and `journey_natural`. Print rows ranked by the objective
(survival high, share low, journey reached), enough per row to act on (the varied params + all
three metrics). `log`/`println!` what was searched and any caps (doc02.01: no silent truncation).

### 5. One fast regression test (`tests/balance.rs` or in `macro_sim.rs`)

Assert the chosen default `ElkParams` sit inside the balanced envelope: `survival ≥ S_floor`,
`journey_natural` reaches the edge band, `mean_migration_share ≤ σ_max`. Use a short fixed
horizon so the test stays a few seconds. Start the thresholds loose (doc02.02).

## Files to Modify

- `src/sim_harness.rs` — NEW. `make_app`, readers, `RunMetrics`, `run_metrics`, `journey_natural`.
- `src/lib.rs` — `pub mod sim_harness;`.
- `tests/macro_sim.rs` — import shared helpers from `sim_harness` instead of local copies.
- `src/bin/sweep.rs` — NEW. The sweep loop + ranked output.
- `tests/balance.rs` — NEW (or add to `macro_sim.rs`). The fast envelope regression test.

## Verification

```sh
cargo build --bin mesopotamia
cargo build --bin demo1
cargo build --bin sweep
cargo test
cargo run --bin sweep   # smoke: prints ranked rows, exits
```

## Out of Scope

- UI, graphs, pie (UI tasks).
- A gradient optimizer beyond grid/random search.
- Tightening macro_sim invariants.

## Notes

- Depends on the `drive-sampling-resource` Surface (`DriveSamples`, `DriveSample::migration_share`)
  and the `drive-decomposition-metric` Surface (`ElkParams.quiet`, residual behaviour). Triage
  against those Surfaces; the code may not exist yet at triage time.
- `ElkParams` has `pub` fields + `Default`; derive `Clone` if not already, to build candidates
  and the migration=0 counterfactual.

## Surface after this phase

- `mesopotamia::sim_harness` — `make_app()`, `run_metrics(ElkParams, ticks) -> RunMetrics`,
  `journey_natural(ElkParams, ticks) -> usize`, and the shared metric readers; `tests/macro_sim.rs`
  consumes it.
- `src/bin/sweep.rs` — runnable sweep printing ranked balanced candidates.
- A fast regression test pinning the default params inside the doc02.02 balanced envelope.
- Negative space: no UI consumption; thresholds deliberately loose.
