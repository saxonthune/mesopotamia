# Probe Scenarios + Decider Boundary

## Motivation

Rung 6 (doc02.03): the simulation's equivalent of a unit test — a minimal hand-built world that
isolates one question. For the elk-water bug: one elk, a strip of forage, one barrier with a single
ford — does it reach the food within N ticks? Run headless and deterministic, it asserts an intent
the way a balance metric does and pins it against future change. Paired with ablation (zeroing
drives one at a time), it attributes behaviour to a cause.

## Do NOT

- Do NOT build a scenario DSL — keep probe specs minimal and code-defined (a small struct + builder).
- Do NOT replace `cross_desire` (`src/elk/movement.rs`) — the probe exercises the real softmax move;
  the extracted kernel stays as-is.
- Do NOT introduce flakiness — probes must seed/fix RNG so assertions are stable.
- Do NOT run Bevy-linking probes under `test-fast`; they belong in the `test-all` integration gate.

## Plan

### 1. Probe-world builder (`src/sim_harness.rs`)

Today `make_app` runs the full `WorldgenPlugin`. Add a way to build a headless app over a
hand-constructed `Grid` instead: e.g. `make_probe_app(grid: Grid, elk: &[(usize /*cell*/, u8 /*slot*/)])`
that inserts the grid, spawns the given elk, and sets `Sim::Running` directly (bypassing
`Generating`/worldgen). Provide a tiny grid constructor for tests: a forage strip, a column of water
with one `is_ford` cell, start positions on the near bank. Confirm `Grid` exposes
constructors/setters (`set_water`, ford tagging, `set_forage`/grass) in `src/grid.rs`; add minimal
test-only setters if missing.

### 2. `Decider` boundary trait (`src/sim_harness.rs` or `src/elk`)

Introduce a thin trait the harness calls to step one decision generically — the one place a trait
earns its keep (per the data-seam-vs-trait analysis). Minimal shape:

```rust
pub trait Decider { fn decide(&self, cell: usize, grid: &Grid, params: &ElkParams) -> Decision; }
```

Back it with the elk's real decision path where practical (reuse `grass_gradient` from phase 2 and
the `Decision` from phase 1). If full neighbour context makes a clean trait impl impractical, scope
the trait to the field-only decision and document the limitation — do not contort `herd_move`.

### 3. Crossing probe + ablation (integration test)

Add a `test-all` integration test: build the crossing probe, run N ticks, assert the (hungry) elk's
`max_col`/cell reaches the far-bank forage band. Add an ablation helper that re-runs the same probe
with one `ElkParams` drive weight zeroed (e.g. `cohesion = 0`) and reports the outcome delta, so a
pinning test can show which drive gates the crossing.

## Files to Modify

- `src/sim_harness.rs` — `make_probe_app`, probe-grid constructor, `Decider` trait, ablation helper.
- `src/grid.rs` — minimal test-only grid setters if not already public.
- `tests/` integration (or the macro-sim test binary) — the crossing probe + ablation assertions.

## Verification

```bash
just test-fast
```

## Out of Scope

- A general scenario DSL or scenario file format.
- UI for probes.

## Notes

- Chain `sim-observability`, phase 6 of 6 (final). Depends on phase 1 (`Decision`, decision path),
  phase 2 (`grass_gradient`), and pairs with phase 5 (same harness). **Triage against their Surface
  blocks.**
- `make_app`/`run_metrics`/`journey_natural`/`max_col_reached` exist in `sim_harness.rs`; model the
  probe app on `make_app` (MinimalPlugins, `Time::<Fixed>`, `StatesPlugin`, the sim plugins).
- Determinism: fix the RNG seed for the probe; `herd_move` uses `rand::rng()` — if it can't be seeded
  externally, run enough ticks that the crossing is statistically certain and assert a band, not an
  exact cell, OR add a test seam to inject a seeded RNG. Prefer a seam if cheap.
- Patterns: doc02.01, doc02.03, verification skill (extract a metric, then assert how it must behave).

## Surface after this phase

- `make_probe_app(...)` + a probe-grid constructor in `sim_harness.rs`.
- A `Decider` trait at the harness boundary, backed by the real decision path (field-scoped if
  necessary, with the limitation documented).
- An ablation helper and a crossing probe integration test; simulation behaviour unchanged.
