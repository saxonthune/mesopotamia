# River crossings: riffles & pools, coupled width, shallows as fords

## Motivation

Water is currently a wall with periodic straight doors: every main channel is
stamped at uniform full depth (1.0), then `stamp_fords` paints **straight
horizontal bands** across it as the only cheap crossings. They look artificial and
elk pile up on the banks between them. Real rivers cross at **riffles** — shallow,
wide, gravelly stretches — between **pools** that are deep and narrow. This phase
varies depth *and* width along each main from one 1-D noise profile (riffle =
shallow+wide, pool = deep+narrow), then tags **shallow water** as fordable so the
crossings follow the river's own crooked path. Elk can cross anywhere but a shallow
riffle costs a fraction of a deep pool, so they prefer shallows on their own.

This is **phase 2 of a 3-phase chain** (tributaries → riffle-crossings → soil-type).
**Triage against phase 1's Surface, not against trunk** — phase 1 (tributaries) has
already merged ahead of you. From that Surface, the pieces you build on:
- `src/river/spec.rs` — `RiverSpec` still has `radius`, `core`, `ford_spacing`,
  `ford_depth`, `confluence_pairs`, lake/oxbow knobs, and (from phase 1)
  `trib_spacing`/`trib_length`. `tributaries` is gone. `Heading`, `RIVER_SEED`,
  `PEN_*` constants present.
- `src/river/mod.rs` — `pub fn generate_water(grid, spec) -> (mains, tribs)`; in its
  loop each main is carved then `rasterize(grid, &cl, spec.radius, spec.core, 1.0)`;
  later `stamp_fords(grid, &mains, spec.radius, spec.ford_spacing, spec.ford_depth)`;
  then `compute_water_prox`. Tests incl. `fords_carry_low_water`,
  `main_channel_has_deep_water`, `water_levels_span_expected_range`,
  `rivers_flow_top_to_bottom`.
- `src/river/raster.rs` — `rasterize(grid, centerline, radius, core, max_depth)`
  (max-merge), plus `stamp_fords`/`ford_indices` and their 3 unit tests (you remove
  these).
- `src/river/cost.rs` — `lerp(a, b, t)` (`pub(super)`).
- `src/field.rs` — `value_noise(width, height, passes, rng)` and `normalize(field)`.
  Calling `value_noise(len, 1, passes, rng)` yields **1-D** smoothed noise of length
  `len` (height 1 → only left/right neighbours).

## Agent execution notes (READ FIRST)

- **Run every cargo/build command in the FOREGROUND and let it block to
  completion.** Do NOT background it and poll with `sleep`/`tail` — this sandbox
  BLOCKS that pattern and you have no polling tool, so you will strand yourself and
  lose your work. The Verification commands use a pre-warmed shared target dir.
- Commit after each logical unit of work; verify `git log --oneline -3` before finishing.

## Do NOT

- Do NOT keep the straight horizontal ford bands. Remove `stamp_fords` and
  `ford_indices` (and their tests) — shallow water becomes the crossing, not a band.
- Do NOT change `src/elk/movement.rs`. The crossing mechanism stays
  `step_water_penalty` + the `ford` flag + `ford_discount`; do NOT lower
  `water_cost`. Riffles + ford-tagged shallows are what make elk cross.
- Do NOT wall off the +x migration axis. `macro_sim::herds_reach_the_far_edge` is
  the canary and MUST stay green. If pools span the whole channel with no riffle gap
  on the migration corridor, raise `riffle_ford_threshold` or `riffle_passes` — do
  NOT touch the test.
- Do NOT touch `src/grid.rs`, `src/worldgen/`, or `src/render.rs`.

## Plan

### 1. Spec fields (src/river/spec.rs)

First `grep -rn "spec.radius\|spec.ford_spacing\|spec.ford_depth" src/` to confirm
these are used only in `generate_water` and `stamp_fords` before removing them.
- Remove `radius`, `ford_spacing`, `ford_depth` from `RiverSpec` and `Default`.
- Add (with `Default`):
  - `riffle_depth: f32` = `0.3` — water level at a full riffle (shallow).
  - `riffle_radius: isize` = `4` — bank radius at a riffle (wide).
  - `pool_radius: isize` = `2` — bank radius at a pool (narrow).
  - `riffle_ford_threshold: f32` = `0.45` — any water cell at/below this is a
    fordable crossing.
  - `riffle_passes: usize` = `6` — 1-D smoothing passes for the riffle profile.
- Keep `core` (still used by mains here and by tributaries/oxbows).

### 2. Variable-depth/width main stamping (src/river/raster.rs)

- Delete `ford_indices`, `stamp_fords`, and their 3 `#[cfg(test)]` tests.
- Add `pub(super) fn stamp_main_channel(grid, centerline: &[usize], profile: &[f32], spec: &RiverSpec)`.
  `profile[t]` in [0,1] is the riffle↔pool value at centerline position `t`
  (0 = riffle, 1 = pool). For each `(t, &center)`:
  - `depth = lerp(spec.riffle_depth, 1.0, profile[t])`.
  - `radius = lerp(spec.riffle_radius as f32, spec.pool_radius as f32, profile[t]).round() as isize`.
  - `rasterize(grid, &[center], radius, spec.core, depth)` (max-merge keeps deeper
    overlaps, so pools stay deep where they meet a riffle edge).
  Import `lerp` via `use super::cost::lerp;`.
- Add `pub(super) fn tag_shallows_as_fords(grid, threshold: f32)`: for every cell,
  `grid.set_ford(i, w > 0.0 && w <= threshold)` where `w = grid.water(i)`. This
  tags riffles (and naturally-shallow tributary/lake-edge cells) as crossings, all
  following the water's real shape — no straight bands.

### 3. Wire into the pipeline (src/river/mod.rs)

In the `generate_water` main loop, replace the uniform
`rasterize(grid, &cl, spec.radius, spec.core, 1.0)` with a per-river riffle profile
+ variable stamping:
```rust
let mut riffle_rng = StdRng::seed_from_u64(RIVER_SEED ^ ((i as u64 + 1) << 16));
let profile = field::normalize(&field::value_noise(cl.len(), 1, spec.riffle_passes, &mut riffle_rng));
raster::stamp_main_channel(grid, &cl, &profile, spec);
```
(`cl.len()` ≥ 1 always; `value_noise` over a length-1 row is fine.) Add
`use crate::field;` if not already imported.

Replace the `stamp_fords(...)` call (after the features block, before
`compute_water_prox`) with `raster::tag_shallows_as_fords(grid, spec.riffle_ford_threshold);`.

### 4. Tests (src/river/mod.rs)

- `fords_carry_low_water`: keep, but assert ford cells satisfy
  `0.0 < g.water(i) <= spec.riffle_ford_threshold` (ford_depth is gone).
- `main_channel_has_deep_water`: still valid (pools reach 1.0) — leave.
- `water_levels_span_expected_range`: still valid — leave.
- Add `river_has_riffles_and_pools`: over the union of main centerlines, assert at
  least one cell has `water <= spec.riffle_depth + EPS` (a riffle) **and** at least
  one has `water >= 0.9` (a pool), and at least one ford cell exists.
- `generation_is_deterministic`, `rivers_flow_top_to_bottom`,
  `confluence_child_joins_parent`, `tributaries_join_a_main_channel`,
  `per_river_params_differ`, `lakes_are_seeded` must stay green (centerline carving
  is unchanged).

## Files to Modify

- `src/river/spec.rs` — remove `radius`/`ford_spacing`/`ford_depth`; add the five riffle knobs; update `Default`.
- `src/river/raster.rs` — remove `ford_indices`/`stamp_fords`(+tests); add `stamp_main_channel` + `tag_shallows_as_fords`.
- `src/river/mod.rs` — riffle profile + `stamp_main_channel` in the loop; swap `stamp_fords` for `tag_shallows_as_fords`; update/add tests.

## Verification

Run in the FOREGROUND.

```bash
CARGO_TARGET_DIR=/home/saxon/.cache/mesopotamia-shared-target cargo test --lib
CARGO_TARGET_DIR=/home/saxon/.cache/mesopotamia-shared-target cargo test --test macro_sim
```

`herds_reach_the_far_edge` inside `macro_sim` is the crossing canary — it must pass.

## Out of Scope

- `src/elk/movement.rs` tuning (water_cost etc.) — explicitly unchanged.
- Soil-type field and two-tone render (phase 3).

## Notes

- If `herds_reach_the_far_edge` fails, the migration corridor lacks a riffle gap:
  raise `riffle_ford_threshold` (e.g. 0.5–0.55) and/or `riffle_passes` so riffle
  stretches are longer/more frequent. Tune the world, never the test.
- The 1-D `profile` length equals the centerline length, so riffles/pools scale with
  each river automatically.

## Surface after this phase

- `RiverSpec`: `radius`, `ford_spacing`, `ford_depth` **removed**; gains
  `riffle_depth`(0.3), `riffle_radius`(4), `pool_radius`(2),
  `riffle_ford_threshold`(0.45), `riffle_passes`(6). `core`, `confluence_pairs`,
  `trib_spacing`/`trib_length`, lake/oxbow/spread knobs all retained.
- Main channels have depth and width that vary along their length (riffle↔pool).
  Cells with `0 < water <= riffle_ford_threshold` are `ford`-flagged crossings that
  follow the water's crooked shape; no straight ford bands remain.
- `raster.rs`: `stamp_fords`/`ford_indices` gone; `stamp_main_channel` +
  `tag_shallows_as_fords` present (`pub(super)`).
- `generate_water(grid, spec) -> (mains, tribs)` signature/return unchanged.
- `src/elk/movement.rs` unchanged (`step_water_penalty`, `ford` flag, `ford_discount`,
  `water_cost` all as before). `src/grid.rs`, `src/worldgen/`, `src/render.rs` untouched.
