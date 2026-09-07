---
name: instrumentation-one-source-many-projections
description: telemetry/stats architecture — one raw MetricLog source, concern-shaped projections; don't add a 4th bespoke collector
metadata:
  type: project
---

The headless stats/telemetry harness is built on **one raw data source, many projections** (decided 2026-06-26):

- **Source** = `metrics::MetricLog` (time × named scalars), filled by the `metrics::WORLD_METRICS` fn-pointer registry (`WorldMetric { name, sample: fn(&mut World)->f32 }`). Same `const` fn-pointer idiom as `TestMap`/`Preset` — **not a trait**, no dyn. Add a stat = one row in `WORLD_METRICS`.
- **Projections** are concern-shaped, NOT one shared trait (their output types differ): `MetricLog::to_csv()` / `summary()` for agent/console, `behaviors::series::{slope,mean,min,max,final_value,first_reaching}` for test assertions, and (future) in-app egui graphs.
- **Runner** = `sim_harness::run(&Scenario, &[WorldMetric]) -> MetricLog`. `Scenario { world: WorldSource, params, ratios, spawns: Vec<(tick,count)>, ticks }` plays a map through the real plugins (via `make_app`), driving `ManualSpawn` on schedule.
- Tests: `tests/scenarios.rs`; run `just probe-scenario` (summary) or `just probe-scenario csv=1` (full CSV).

**Why / how to apply:** the unification is the DATA TYPE, not a projection interface — forcing UI + console + asserts under one `Projection` trait is the false-unification to avoid (user dislikes OO hierarchies, see [[user-dislikes-fowler-and-oop]]). When adding stats output, project the one `MetricLog`; do NOT add a new bespoke time-series collector. `History` (live UI ring buffer), `RunTrace`, `BehaviorTrace` predate this and should migrate onto `MetricLog` opportunistically — not be joined by a fourth. See [[green-wave-removed-restarting-direction]] for the test-map harness this rides on. Caveat of the `#[path]` demo1 bin: `metrics.rs` now pulls `diagnostics`+`behaviors`, so both are re-included in `src/bin/demo1.rs` (see [[demo1-bin-duplicates-modules-via-path]]).
