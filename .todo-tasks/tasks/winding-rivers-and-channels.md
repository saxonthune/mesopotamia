# Windier Rivers, Tributaries & Oxbows (world-gen A)

## Motivation

The river is a single smooth least-cost channel (`SMOOTH_PASSES = 8` → broad gentle
bends), and the grass band mirrors it, so the elk journey reads as a straight march
down a symmetric green lane. A more sinuous main channel, narrow shallow tributaries,
and off-channel oxbow pools make the green band weave and create islands, pinch-points,
and oases that force herds to navigate.

This is **Phase A of a two-phase chain.** Reconciled against current code: water is
*already* a movement cost — `herd_move` (`src/elk.rs:461-462`) subtracts
`grid.water(next) * params.water_cost`, so the river's shape already governs where
herds can go. Reshaping the river therefore directly changes herd movement *today*.
This phase also **authors the ford mask** (`Grid::is_ford`) that Phase B
(`water-as-barrier-fords`) consumes for its crossing discount and swim cost.

## Do NOT

- **Do not touch `src/elk.rs`, `herd_move`, or `ElkParams`.** Movement is Phase B's
  territory. `ElkParams.water_cost` (default 2.0) stays wired exactly as it is.
- **Do not make tributaries or oxbows deep.** They must carry a LOW water level and be
  narrow so they read as small streams/pools and stay cheap to cross under the existing
  `grid.water(next) * water_cost` penalty. Only the main channel runs deep.
- **Do not add live UI controls or a runtime "regenerate river" button.** Meander and
  channel count are generation-time config (named consts) read once at `Startup`.
- **Do not introduce clock or extra random entropy.** Generation stays a deterministic
  function of `RIVER_SEED` (and any derived sub-seeds). Same seed → same world.
- **Do not change `compute_water_prox` semantics or the grass/`capacity` model.** It must
  keep deriving capacity from any `water > 0` (tributaries/fords/oxbows included).

## Plan

### 1. Make meander tunable and lower the default

In `src/river.rs`, rename the meander control to a clearly-named generation-time const
(e.g. `MEANDER_PASSES`) and lower its default from 8 to ~4 so the main channel snakes
(fewer smoothing passes → tighter wiggles in the cost field). Keep it a `const`; no UI.

### 2. Reuse `field::smooth` for the cost field

Rewrite `cost_field` to build the white-noise field, call
`field::smooth(&white, grid.width(), grid.height(), passes)`, then map the smoothed
field to `u32` cost (keep the existing `(v * 1000.0) as u32 + 1`). This removes the
hand-rolled box-blur that duplicates `field::smooth` exactly.

### 3. Multi-channel carve (shallow tributaries)

Carve the main channel as today (full `RIVER_RADIUS`, deep core via `rasterize`), then
carve **2 tributaries** through the *same* cost field from distinct start/goal endpoints
chosen from the river RNG (deterministic). Rasterize tributaries with a **smaller radius**
and a **reduced water level** (a depth scale well below 1, e.g. ~0.35) so they are narrow
and shallow — "small streams." Where channels nearly touch, islands and pinch-points
emerge naturally. Keep `rasterize`'s `level > grid.water(cell)` max-merge so overlaps
take the deeper value.

### 4. Oxbows / detached pools

Seed 1–3 off-channel shallow pools: pick local minima of the cost field that are not
adjacent to any carved channel cell, and rasterize a small shallow pool (low water level,
small radius) at each. Selection is deterministic from the river RNG.

### 5. Ford mask — periodic shallow crossings on the main channel

Add per-cell ford storage to `Grid` and author fords along the **main channel only**:

- A pure helper, e.g. `ford_indices(centerline: &[usize], spacing: usize) -> Vec<usize>`,
  returns every `spacing`-th centerline cell. Unit-testable in isolation.
- For each returned centerline cell, **shallow a short crossing band** across the channel
  width (set those cells' water to a low level, overriding the deep core there) and mark
  them `is_ford = true`.

Fords live only on the deep main channel — tributaries/oxbows are already shallow and
cheap, so they need no fords.

## Files to Modify

- `src/grid.rs` — add a `ford: Vec<bool>` field to `Grid`; initialise `vec![false; width*height]`
  in `Grid::new`; add `is_ford(&self, index: usize) -> bool` and
  `set_ford(&mut self, index: usize, value: bool)` accessors (mirror the `water`/`set_water` pair).
- `src/river.rs` — `MEANDER_PASSES` const; `cost_field` via `field::smooth`; multi-channel
  carve with shallow narrow tributaries; oxbow seeding; ford authoring via the `ford_indices`
  helper; `generate_river` orchestrates all of it at `Startup`.
- `src/river.rs` (tests) — co-located `#[cfg(test)]` module.

## Verification

```bash
cargo test
cargo build
cargo clippy -- -D warnings
```

Tests (extract pure helpers, assert example + structural properties — see the
metamorphic-metric-testing memory): `ford_indices` spacing/coverage; a determinism check
that generating twice from the same seed yields identical `water` fields; an assertion
that tributary/oxbow cells carry strictly lower water than the main-channel core.

## Out of Scope

- Elk movement, the ford *discount*, swim energy cost, and `ElkParams` — all Phase B
  (`water-as-barrier-fords`).
- Live UI exposure of meander/channel-count (a later task if wanted).
- Vegetation / browse changes (separate C/D work).
- Visual styling to distinguish fords/tributaries (render already lerps colour by water
  level, so shallow cells read as lighter blue automatically).

## Notes

- `src/river.rs` and the water logic in `src/grid.rs` are co-owned with the procgen
  session — this phase is the one editing them; Phase B does not touch them, which is why
  the chain has no merge conflict.
- `field::smooth` already matches `cost_field`'s box-blur exactly.

## Surface after this phase

- `Grid::is_ford(index) -> bool` and `Grid::set_ford(index, bool)` exist. `is_ford` is
  `true` exactly on authored ford cells: crossing bands spaced periodically along the
  **main channel** centerline, and nowhere else.
- Ford cells, tributaries, and oxbow pools all carry a LOW `grid.water` level (shallow);
  the main channel core stays deep (~1.0). `grid.water(index)` remains in `[0, 1]`:
  0 = dry, higher = deeper.
- Because tributaries/oxbows/fords are shallow, the existing scoring term
  `grid.water(next) * params.water_cost` already makes them comparatively cheap to cross;
  the deep main channel is the barrier, fords the cheap line across it.
- The river may be several braided channels plus detached oxbow pools. `grid.water`
  stays the single source of "where/how deep is water," and `compute_water_prox` still
  derives grass capacity from any `water > 0`.
- **Negative space:** `src/elk.rs`, `herd_move`, and `ElkParams` are UNCHANGED by this
  phase. `ElkParams.water_cost` (default 2.0) stays wired exactly as today. `is_ford` is
  authored but NOT yet read by movement — Phase B adds the ford discount and swim cost.
