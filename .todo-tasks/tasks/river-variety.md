# River variety (per-river spread + lakes)

## Motivation

The three main rivers are parallel clones: every river shares the global
`spec.drift` and `spec.bendiness`, so they lean the same way and wander the same
amount. The only standing water beyond the channels is the tiny oxbow pools.
This phase makes each river subtly distinct and adds a few larger lakes, all
through the existing seeded, pure, deterministic generator (`RiverSpec` +
`RIVER_SEED`, anisotropic `carve` + `rasterize`). Topology is unchanged — rivers
still run top→bottom independently; phase B handles merging.

## Do NOT

- Do NOT change channel topology (no merging/deltas) — that is phase B.
- Do NOT vary the per-river *smoothing passes* (wavelength): the cost field is
  shared across all rivers by design ("one shared cost field for all channels").
  Per-river bendiness varies only the directional `penalty` used in that river's
  `carve`, never `cost_field`'s `passes`.
- Do NOT sample per-river jitter from the main `rng` stream — that would shift
  oxbow/tributary placement. Derive it from a per-river child RNG seeded from
  `RIVER_SEED` and the river index, so existing structures stay put.
- Do NOT make the spread large: keep drift strictly positive (rivers must still
  drift rightward) and keep the +x migration axis crossable (the macro crossing
  test is the canary).
- Do NOT seed generation from the clock — `StdRng` only.

## Plan

### 1. Per-river drift + bendiness spread (subtle, ±~20%)

In `generate_river_inner` (`src/river.rs`), the main-river loop currently uses
the global `spec.drift` for the exit offset and a single `penalty` (derived from
`spec.bendiness`) computed once before the loop. Make drift and penalty per-river:

- Add two `RiverSpec` fields with subtle defaults:
  - `drift_spread: f32` (default `0.2`) — fractional jitter applied to drift.
  - `bendiness_spread: f32` (default `0.2`) — additive jitter applied to bendiness.
- Inside the loop, for river `i`, derive a deterministic child RNG, e.g.
  `let mut rrng = StdRng::seed_from_u64(RIVER_SEED ^ (i as u64 + 1));`
  and sample:
  - `drift_i = spec.drift * (1.0 + rrng.random_range(-spec.drift_spread..=spec.drift_spread))`
    — clamp so it stays `> 0`.
  - `bendiness_i = (spec.bendiness + rrng.random_range(-spec.bendiness_spread..=spec.bendiness_spread)).clamp(0.0, 1.0)`,
    then `penalty_i = lerp(PEN_HIGH as f32, PEN_LOW as f32, bendiness_i) as u32`.
- Use `drift_i` for that river's `exit_col` and `penalty_i` for its `carve` bias.
- Keep the shared `cost_field` (and its global `passes`) exactly as is.

### 2. Lakes / wetlands (count-based, seeded at deepest basins)

After the oxbow block (which seeds small pools at local cost minima), add a lake
pass that places a few larger standing-water bodies at the *deepest* cost-field
basins:

- Add `RiverSpec` fields: `lake_count: usize` (default `3`), `lake_radius: isize`
  (default `4`), `lake_core: isize` (default `2`).
- Reuse the oxbow candidate predicate (non-water local minima not adjacent to a
  carved channel cell). From those candidates, pick the `lake_count` with the
  **lowest `cost[i]`** (deepest basins) rather than random — deterministic and
  visually intentional. Break ties by index for stability.
- `rasterize` each at `lake_radius` / `lake_core` with `max_depth = 1.0` (full
  standing water). The existing `compute_water_prox` pass (still last) gives them
  a broad riparian grass halo automatically.

## Files to Modify

- `src/river.rs` — `RiverSpec` (5 new fields + defaults), per-river drift/penalty
  in the main loop, the lake pass after oxbows.
- `src/river.rs` tests — add: `per_river_params_differ` (two rivers get different
  drift/penalty-derived behavior, e.g. their centerlines are not identical) and
  `lakes_are_seeded` (with `lake_count > 0`, deep standing-water cells exist away
  from the main centerlines). Keep `generation_is_deterministic` green.

## Verification

```bash
cargo test --lib
cargo test --test macro_sim
cargo build --bin mesopotamia
```

## Out of Scope

- River merging / confluence / delta (phase B).
- Regional grass fertility (phase C).
- Elk/movement changes.

## Notes

- Watch `macro_sim::herds_reach_the_far_edge`: lakes add fordless water. Place
  lakes off the channels but the budget (24 ticks/col) has margin; if it fails,
  reduce `lake_count`/`lake_radius` rather than touching the test threshold.
- `RiverSpec` stays the single source of truth for every knob (doc02.01).

## Surface after this phase

- `RiverSpec` (in `src/river.rs`) gains fields, all with defaults:
  `drift_spread: f32` (0.2), `bendiness_spread: f32` (0.2), `lake_count: usize`
  (3), `lake_radius: isize` (4), `lake_core: isize` (2). All pre-existing
  `RiverSpec` fields are unchanged.
- `generate_river_inner(grid: &mut Grid, spec: &RiverSpec) -> (Vec<Vec<usize>>, Vec<Vec<usize>>)`
  keeps its signature and still returns `(mains, tribs)`. `mains.len() == spec.count`,
  and **every** main centerline still runs from row 0 (entry, `cl.last()`) to row
  `height-1` (exit, `cl[0]`) — topology is unchanged in this phase.
- Each main river now carves with its own drift and directional penalty, derived
  deterministically from `RIVER_SEED` + river index (not from the main rng stream).
- Lakes are full-depth standing water placed at the deepest non-channel basins.
- Oxbows, tributaries, fords, `compute_water_prox`, and `cost_field` (shared,
  global `passes`) all still exist and behave as before.
