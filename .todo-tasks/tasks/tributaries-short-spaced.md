# Tributaries: short feeders spaced along each main

## Motivation

Today's tributaries source at the map's left/right **edges** and carve all the way
to a confluence on a main — so they read as long cross-map streams, not feeders.
Real dendritic drainage is the opposite: many **short** streams, **spaced out**
along the trunk, each dropping in from nearby higher ground. This phase reworks the
tributary algorithm to produce that look.

This is **phase 1 of a 3-phase chain** (tributaries → riffle-crossings → soil-type).
`src/river/` is exactly as on trunk after the worldgen refactor. Triage against this
live layout:
- `src/river/spec.rs` — `RiverSpec` with fields incl. `count`, `drift`, `radius`,
  `core`, `tributaries`, `trib_radius`, `trib_depth`, `ford_spacing`, `ford_depth`,
  lake/oxbow/spread fields, and `confluence_pairs`. Plus `Heading`, `RIVER_SEED`,
  `PEN_HIGH/PEN_LOW`.
- `src/river/features.rs` — `carve_tributaries(grid, cost, rng, spec, mains) ->
  Vec<Vec<usize>>` (the function you rewrite), plus `place_oxbows`, `place_lakes`,
  `is_dry_basin`. All `pub(super)`.
- `src/river/cost.rs` — `carve(grid, cost, start, goal, bias)` (least-cost; pass
  `None` for no heading bias) and `lerp`. `pub(super)`.
- `src/river/raster.rs` — `rasterize(grid, centerline, radius, core, max_depth)`.
- `src/river/mod.rs` — `pub fn generate_water(grid, spec) -> (mains, tribs)`; the
  call `features::carve_tributaries(grid, &cost, &mut rng, spec, &mains)`; the tests
  incl. `tributaries_join_a_main_channel`.

## Agent execution notes (READ FIRST)

- **Run every cargo/build command in the FOREGROUND and let it block to
  completion.** Do NOT background it and poll with `sleep`/`tail` — this sandbox
  BLOCKS that pattern and you have no polling tool, so you will strand yourself and
  lose your work. The Verification commands use a pre-warmed shared target dir, so
  compiles take seconds.
- Commit after each logical unit of work; verify with `git log --oneline -3` before
  finishing.

## Do NOT

- Do NOT source feeders at the map edges any more — that is the long-stream bug.
- Do NOT carve feeders longer than a short bound (see `trib_length`). A feeder that
  wanders half the map is wrong even if it joins a main.
- Do NOT touch `src/grid.rs`, `src/worldgen/`, `src/render.rs`, or elk/movement.
- Do NOT use the wall clock for any sampling — use the seeded `rng` passed in.

## Plan

### 1. Spec fields

In `src/river/spec.rs`, on `RiverSpec`:
- Remove `tributaries: usize`.
- Add `trib_spacing: usize` — cells between successive feeders along a main
  centerline. Default `24`.
- Add `trib_length: isize` — the lateral source offset / nominal feeder length in
  cells. Default `12`.
- Keep `trib_radius`, `trib_depth`, `core`.
Update `Default` accordingly (drop `tributaries`, add the two new fields).

### 2. Rewrite `carve_tributaries` (src/river/features.rs)

Produce **short, spaced** feeders. For **each** main centerline:
- Walk the centerline by index, stepping `trib_spacing` positions at a time
  (`.step_by`), skipping the first/last ~10% so feeders join the interior, not the
  source/mouth. Each stepped cell is a **confluence point** on the main.
- For each confluence point at `(col, row)`, choose a **short source** offset
  ~`trib_length` cells to one side and slightly upstream (smaller row): e.g.
  `source = (col ± trib_length, row - trib_length/2)`. Pick the side with the
  seeded `rng` (or alternate). If the source falls off-grid, try the other side; if
  both are off-grid or the source already sits in water (`grid.water(src) > 0.0`),
  skip this feeder.
- Carve `carve(grid, cost, source_idx, confluence_idx, None)` (least-cost, no
  heading bias — naturally crooked and short because source and goal are close),
  then `rasterize(grid, &tcl, spec.trib_radius, spec.core, spec.trib_depth)`.
- Collect each `tcl` into the returned `Vec`.

The signature stays `carve_tributaries(grid, cost, rng, spec, mains) ->
Vec<Vec<usize>>`. Keep the early-return when `mains.is_empty()`.

### 3. Update tests (src/river/mod.rs)

`tributaries_join_a_main_channel`:
- Remove the `tribs.len() == spec.tributaries` assertion (that field is gone).
- Keep asserting each tributary's confluence cell (`tcl[0]`) lies on a main
  centerline (set-membership).
- Add: assert `!tribs.is_empty()`, and that **every** feeder is short —
  `tcl.len() <= 4 * spec.trib_length as usize` (least-cost may wiggle, but a short
  feeder never approaches map height). This is the regression guard for the
  long-stream bug.

`water_levels_span_expected_range` should still pass (shallow tributary cells exist).

## Files to Modify

- `src/river/spec.rs` — drop `tributaries`; add `trib_spacing`, `trib_length`; update `Default`.
- `src/river/features.rs` — rewrite `carve_tributaries` (short, spaced feeders).
- `src/river/mod.rs` — update `tributaries_join_a_main_channel` (drop count assert, add short-length assert).

## Verification

Run in the FOREGROUND. `CARGO_TARGET_DIR` points at a pre-warmed shared target so
compiles take seconds.

```bash
CARGO_TARGET_DIR=/home/saxon/.cache/mesopotamia-shared-target cargo test --lib
CARGO_TARGET_DIR=/home/saxon/.cache/mesopotamia-shared-target cargo test --test macro_sim
```

## Out of Scope

- Riffle/pool depth, varying main width, ford reform (phase 2).
- Soil-type field and two-tone render (phase 3).

## Notes

- `is_dry_basin` and the oxbow/lake code are unrelated — leave them.
- Determinism: `generation_is_deterministic` must stay green; only the seeded `rng`
  may drive side/source choices.

## Surface after this phase

- `RiverSpec` (src/river/spec.rs): `tributaries` **removed**; gains
  `trib_spacing: usize` (default 24) and `trib_length: isize` (default 12). All
  other fields unchanged, including `radius`, `core`, `ford_spacing`, `ford_depth`,
  `confluence_pairs`, and the lake/oxbow/spread knobs.
- `carve_tributaries(grid, cost, rng, spec, mains) -> Vec<Vec<usize>>` — signature
  unchanged; now emits multiple **short** feeders spaced ~`trib_spacing` along each
  main, each ending on a main centerline, each `<= 4*trib_length` cells long.
- `generate_water(grid, spec) -> (mains, tribs)` — signature/return unchanged.
- **Unchanged and still relied on by phase 2**: main-channel carving + uniform
  `rasterize(grid, &cl, spec.radius, spec.core, 1.0)` in the `generate_water` loop;
  `stamp_fords`/`ford_indices` in `raster.rs` and the straight ford bands; `carve`,
  `rasterize`, `compute_water_prox`; lakes, oxbows, confluence routing.
- `src/grid.rs`, `src/worldgen/`, `src/render.rs`, elk/movement untouched.
