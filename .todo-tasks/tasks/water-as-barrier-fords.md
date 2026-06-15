# Water as a Movement Barrier, with Fords (world-gen B)

## Motivation

Water repelling elk is **already implemented**: `herd_move` (`src/elk.rs:461-462`)
scores each candidate step as `desire.dot(step) - grid.water(next) * params.water_cost`,
and `ElkParams.water_cost` (default 2.0) exists. Because the penalty scales with water
level, deep water already repels and shallow tributaries are already cheap.

What is missing — and what this phase adds — is the part that turns a flat detour into a
legible crossing event: **fords** (sanctioned cheap crossings on the deep main channel),
a **swim energy cost** (crossing deep water is a lasting risk, not just a one-step
penalty), and **UI controls** to tune all of it.

This is **Phase B of a two-phase chain.** Phase A (`winding-rivers-and-channels`) authors
the ford mask and the shallow tributaries/oxbows this phase relies on.

## Triage basis — Phase A's Surface (not live code)

Phase A has not merged. Triage against its declared Surface, treating these as given:

- `Grid::is_ford(index: usize) -> bool` exists; `true` exactly on ford cells (periodic
  crossing bands on the main channel), `false` elsewhere.
- Ford cells, tributaries, and oxbows carry LOW `grid.water`; the main channel core is
  deep (~1.0). `grid.water(index)` in `[0, 1]`, higher = deeper.
- `ElkParams.water_cost` (default 2.0) is wired into `herd_move` scoring exactly as today.

If a symbol is not in that list, treat it as nonexistent.

## Do NOT

- **Do not touch river/grid GENERATION** — `src/river.rs`, the ford authoring, the water
  field, `compute_water_prox`. Consume A's Surface only: *read* `Grid::is_ford`; never
  author it. (This is what keeps the chain conflict-free.)
- **Do not re-implement the water penalty from scratch.** It already exists at
  `elk.rs:461-462` — extend that single expression, don't add a parallel one.
- **Do not penalize fords or shallow tributaries heavily.** A ford should be nearly free
  to cross; tributaries are already cheap via their low water level. The drama comes from
  the deep main channel, not from taxing every wet cell.
- **Do not add clock or random entropy**; keep movement deterministic given the RNG.
- **Do not change grass/graze/browse logic** beyond applying the swim energy drain.

## Plan

### 1. Pure step-penalty helper

Add a pure function in `src/elk.rs`, e.g.
`step_water_penalty(water: f32, is_ford: bool, water_cost: f32, ford_discount: f32) -> f32`.
Off a ford it returns `water * water_cost` (today's behaviour); on a ford it returns that
scaled by `ford_discount` (near 0 → a ford is nearly free). Keep it total and pure so it
unit-tests in isolation.

### 2. Wire it into scoring

At `elk.rs:461-462`, replace `- grid.water(next) * params.water_cost` with
`- step_water_penalty(grid.water(next), grid.is_ford(next), params.water_cost, params.ford_discount)`.
Behaviour off fords is identical to today; fords become the cheap crossing line.

### 3. Swim energy cost

Add `ElkParams.swim_drain` (small default, e.g. 0.01). In `herd_move`, **after** an elk
commits to its step (after the pick loop sets `elk.cell`), if the entered cell is deep
water and NOT a ford, subtract `swim_drain * grid.water(elk.cell)` from `elk.energy`
(clamp at ≥ 0). Fords are exempt — crossing at a ford costs no swim energy. `elk` is
already `&mut` in that loop, so no signature change.

### 4. Expose the water knobs in the UI

In `src/ui.rs` `behaviour_tab`, add sliders following the existing `slider(...)` pattern:
`water_cost` (0..=4), `ford_discount` (0..=1), `swim_drain` (0..=0.05). `water_cost` is
currently unexposed; surface it alongside the two new params.

### 5. Extend `ElkParams`

Add `ford_discount` and `swim_drain` fields with defaults in the `Default for ElkParams`
impl (`elk.rs:327`). Suggested: `ford_discount: 0.1`, `swim_drain: 0.01`.

## Files to Modify

- `src/elk.rs` — `step_water_penalty` helper + co-located tests; wire it into `herd_move`
  scoring; swim energy drain after the step pick; `ElkParams` gains `ford_discount` and
  `swim_drain` (struct + `Default`).
- `src/ui.rs` — three sliders in `behaviour_tab`.

## Verification

```bash
cargo test
cargo build
cargo clippy -- -D warnings
```

Tests (pure helper, example + monotonicity — see the metamorphic-metric-testing memory):
`step_water_penalty` returns ~0 on a ford regardless of water; off a ford it is monotonic
increasing in `water`; deep non-ford water costs strictly more than a shallow tributary.

## Out of Scope

- River generation and ford authoring — Phase A (`winding-rivers-and-channels`).
- Vegetation / browse changes (separate C/D work).
- Visual styling to mark fords (render already colours by water level).

## Notes

- This phase touches only `src/elk.rs` and `src/ui.rs` — no overlap with Phase A's
  `src/river.rs` / `src/grid.rs` generation edits, so the chain merges cleanly.
- The existing `water_cost` term is load-bearing; the only change to off-ford behaviour is
  routing it through the helper (numerically identical when `is_ford` is false).

## Surface after this phase

- `herd_move` step scoring penalizes deep water (`water_cost`), discounts fords
  (`ford_discount` → near-free crossing), and drains `swim_drain` energy when an elk enters
  deep non-ford water.
- `ElkParams` gains `ford_discount` and `swim_drain`; `water_cost`, `ford_discount`, and
  `swim_drain` are all exposed in the behaviour UI tab.
- `Grid::is_ford` is now consumed by movement. Generation (Phase A) is unchanged by this
  phase.
