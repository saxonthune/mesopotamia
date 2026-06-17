# Soil type: riparian vs steppe, with two-tone dirt

## Motivation

The bare ground renders as one flat tan, and the simulation has no notion that soil
near water differs from soil far away. Real terrain has a **riparian** band —
dark, moist, alluvial soil hugging the channels — grading out to pale, dry
**steppe** where the shrubs live. This phase adds a real `soil_type` field authored
in world-gen from the water-proximity gradient, gives it a *gentle* ecological nudge
(steppe favours browse, riparian favours grass), and renders bare ground as a
two-tone dirt that visibly ties the wet/dry split to where the shrubs grow.

This is **phase 3 (final) of a 3-phase chain** (tributaries → riffle-crossings →
soil-type). **Triage against phase 2's Surface** — phases 1–2 have merged ahead of
you. The pieces you build on:
- `src/worldgen/mod.rs` — `WorldgenPlugin`, `WorldSpec`, and the orchestrator
  `generate_world(grid)` that runs, in order: `generate_water(&mut grid, &spec.river)`
  → `soil::seed_soil(&mut grid)` → `vegetation::seed_browse_cap(&mut grid)`.
  `mod soil; mod vegetation;` declared here.
- `src/worldgen/soil.rs` — `seed_soil(grid)` (`pub(super)`), authors `soil` from
  fine×macro noise.
- `src/worldgen/vegetation.rs` — `seed_browse_cap(grid)` (`pub(super)`): browse
  capacity = `dry * MAX_BROWSE` on dry, non-water, dense-noise cells, where
  `dry = 1.0 - grid.water_prox(i)`.
- `src/grid.rs` — `Grid` holds `grass/poop/water/water_prox/soil/browse/browse_cap/
  ford` Vecs. Accessors incl. `water_prox(i)`, `soil(i)`, `set_soil`, `water(i)`,
  `set_browse_cap`, `capacity(i) = water_prox[i] * soil[i] * MAX_GRASS`. `Grid::new`
  initialises every field Vec.
- `src/render.rs` — `sync_tiles` colours each cell: `dirt (0.76,0.68,0.48)` →
  `green (0.2,0.7,0.3)` by grass, then → `water (0.1,0.3,0.7)` by water level. Reads
  `Res<Grid>`.

## Agent execution notes (READ FIRST)

- **Run every cargo/build command in the FOREGROUND and let it block to
  completion.** Do NOT background it and poll with `sleep`/`tail` — this sandbox
  BLOCKS that pattern and you have no polling tool, so you will strand yourself and
  lose your work. The Verification commands use a pre-warmed shared target dir.
- Commit after each logical unit of work; verify `git log --oneline -3` before finishing.

## Do NOT

- Do NOT let the ecology nudge break balance. The two-tone render is the primary
  deliverable; the capacity nudge must be **gentle**. If `macro_sim` or `balance`
  goes out of envelope, **reduce the nudge strength toward zero** until both pass —
  do NOT touch the tests or their thresholds.
- Do NOT make `soil_type` a second multiplier on grass `capacity` (that double-counts
  `water_prox` and will swing balance). Keep the grass side untouched in `capacity`;
  apply the nudge only on the **browse** side (see Plan 3).
- Do NOT touch `src/river/`, `src/elk/`, or the worldgen layer ordering of
  water-before-soil-before-vegetation.

## Plan

### 1. `soil_type` field on Grid (src/grid.rs)

- Add `soil_type: Vec<f32>` to `Grid`; init `vec![0.0; width*height]` in `Grid::new`
  (0 = steppe, 1 = riparian).
- Add `pub fn soil_type(&self, index) -> f32` and
  `pub fn set_soil_type(&mut self, index, value)` (clamp [0,1]).
- Do NOT change `capacity`.

### 2. World-gen soil-type layer (src/worldgen/soil_type.rs + mod.rs)

- New `src/worldgen/soil_type.rs` with `pub(super) fn seed_soil_type(grid: &mut Grid)`:
  derive riparian-ness from the water-proximity field the water layer already laid
  down. A smooth band hugging water reads well — e.g.
  `let t = ((grid.water_prox(i) - LO) / (HI - LO)).clamp(0.0, 1.0); grid.set_soil_type(i, t);`
  with `const LO: f32 = 0.2; const HI: f32 = 0.7;` (near-water cells → ~1 riparian,
  far cells → 0 steppe). Water cells themselves can stay whatever the band gives.
- In `src/worldgen/mod.rs`: add `mod soil_type;` and call
  `soil_type::seed_soil_type(&mut grid);` in `generate_world` **after**
  `seed_soil` and **before** `seed_browse_cap` (vegetation may read it).

### 3. Gentle ecological nudge (src/worldgen/vegetation.rs)

Browse already wants dry ground. Reinforce it *gently* with soil type: scale the
browse capacity by a steppe factor that never exceeds 1, e.g.
`let steppe = 1.0 - 0.5 * grid.soil_type(i);` and multiply the existing
`dry * MAX_BROWSE` by `steppe`. This nudges shrubs off the riparian band without
adding or removing forage wholesale. (The `0.5` is the nudge strength — lower it if
balance complains.)

### 4. Two-tone dirt (src/render.rs)

In `sync_tiles`, replace the single `dirt` tone with a soil-type lerp **before** the
grass/water lerps:
```rust
let steppe_dirt = Vec3::new(0.80, 0.72, 0.52);   // pale dry
let riparian_dirt = Vec3::new(0.45, 0.38, 0.26); // dark moist
let dirt = steppe_dirt.lerp(riparian_dirt, grid.soil_type(tile.index));
```
then keep the existing `dirt.lerp(green, h)` and `.lerp(water, w)` steps. No other
render change.

### 5. Test (src/worldgen/soil_type.rs)

Add a `#[cfg(test)]` test `riparian_hugs_water`: build a `Grid`, set `water_prox`
high on a few cells and low elsewhere, run `seed_soil_type`, and assert the
high-prox cells get a higher `soil_type` than the low-prox cells, and all values are
in [0,1].

## Files to Modify

- `src/grid.rs` — `soil_type` Vec + init + `soil_type`/`set_soil_type` accessors.
- `src/worldgen/soil_type.rs` — NEW: `seed_soil_type` + test.
- `src/worldgen/mod.rs` — `mod soil_type;` + orchestrator call after soil, before vegetation.
- `src/worldgen/vegetation.rs` — gentle steppe factor on browse capacity.
- `src/render.rs` — two-tone dirt lerp in `sync_tiles`.

## Verification

Run in the FOREGROUND. Soil/capacity touch ecology, so run the **broad** suite too.

```bash
CARGO_TARGET_DIR=/home/saxon/.cache/mesopotamia-shared-target cargo test --lib
CARGO_TARGET_DIR=/home/saxon/.cache/mesopotamia-shared-target cargo test --test macro_sim
CARGO_TARGET_DIR=/home/saxon/.cache/mesopotamia-shared-target cargo test --test balance
```

All must pass. If `macro_sim`/`balance` regress, lower the nudge strength in Plan 3
toward zero and re-run.

## Out of Scope

- River shape, riffles, tributaries (phases 1–2).
- Any elk/movement change.
- A separate authored noise for soil type (deriving from `water_prox` is enough).

## Notes

- The render reads `grid.soil_type` every frame; it's a plain field read, no cost concern.
- Determinism holds automatically — `seed_soil_type` is a pure function of `water_prox`.

## Surface after this phase

- `Grid` gains `soil_type: Vec<f32>` with `soil_type(i)` / `set_soil_type(i, v)`;
  `capacity` unchanged.
- World-gen runs water → soil → **soil_type** → vegetation; `seed_soil_type` authors
  the riparian/steppe field from `water_prox`.
- Browse capacity is gently biased toward steppe; grass capacity unchanged.
- Bare ground renders two-tone (pale steppe ↔ dark riparian) by `soil_type`.
- All suites green: `cargo test --lib`, `macro_sim`, `balance`.
