# Regional fertility (coarse macro-octave on soil)

## Motivation

The map's grass reads as uniform even though a low-frequency `soil` fertility
field already exists (`seed_soil` in `src/grid.rs`: `SOIL_SEED`, `SOIL_PASSES=6`,
`SOIL_FLOOR=0.35`, mapped into `capacity = water_prox · soil · MAX_GRASS`). The
existing field is "patchy" but not "regional" — there are no broad lush vs sparse
zones to give herds a reason to prefer one part of the map. This phase layers a
second, much-lower-frequency macro-octave on top of the existing fine patches so
the world has genuine regional character.

This phase is independent of the river phases (different concept, different
files). It triages against live `grid.rs`/`field.rs` code on trunk.

## Do NOT

- Do NOT remove or replace the existing fine soil patches — layer the macro
  octave *on top of* them (fine detail inside coarse regions).
- Do NOT seed the macro field from the same `SOIL_SEED` — use a distinct seed so
  the coarse regions are uncorrelated with the fine patches.
- Do NOT seed from the clock — `StdRng` only.
- Do NOT touch river/water generation or elk/movement.

## Plan

### 1. Add the macro-octave constants

In `src/grid.rs`, near the existing soil constants, add:
- `MACRO_SEED: u64` — distinct from `SOIL_SEED`/`BROWSE_SEED`/`RIVER_SEED`.
- `MACRO_PASSES: usize` — much larger than `SOIL_PASSES` (e.g. `14`) so features
  span large regions of the 256×108 grid.
- `MACRO_FLOOR: f32` (e.g. `0.4`) — the sparsest region still carries this
  fraction of its local fertility, so no region is dead.

### 2. Blend the macro octave into soil

In `seed_soil`, after computing the existing fine `patch`
(`normalize(value_noise(..., SOIL_PASSES, ...))`), compute a second macro field
from a fresh RNG seeded with `MACRO_SEED`:
`macro = normalize(value_noise(width, height, MACRO_PASSES, &mut macro_rng))`.
Combine multiplicatively so coarse regions modulate the fine patches:

```
let fine   = SOIL_FLOOR + (1.0 - SOIL_FLOOR) * patch[i];
let region = MACRO_FLOOR + (1.0 - MACRO_FLOOR) * macro[i];
grid.set_soil(i, fine * region);
```

Both factors are in (0, 1], so `soil` stays in range; `capacity` (which also
multiplies `water_prox`) automatically inherits the regional variation.

### 3. Test

Add a focused test (in a `grid.rs` `#[cfg(test)]` module, or extend an existing
one) that builds a grid, runs `seed_soil`, and asserts the soil field has broad
regional spread — e.g. the mean soil of the left third differs from the mean soil
of the right third by more than a small epsilon for the default seed, OR simply
that `min < some threshold < max` with substantial range. Keep it deterministic.

## Files to Modify

- `src/grid.rs` — macro constants, the blend in `seed_soil`, the regional-spread
  test.

## Verification

```bash
cargo test --lib
cargo test --test macro_sim
cargo build --bin mesopotamia
```

## Out of Scope

- River variety / confluence (phases A, B).
- Changing the `capacity` formula shape (still `water_prox · soil · MAX_GRASS`).
- Elk/movement changes.

## Notes

- Lowering overall fertility reduces total carrying capacity. The macro_sim grass
  floor is `2% of Σ capacity` and the crossing budget scales with width, so both
  track the change automatically — but run them: if grazing balance shifts enough
  to collapse or explode the population, raise `MACRO_FLOOR` rather than weakening
  the regional contrast.
- `value_noise` + `normalize` are pure and already tested in `field.rs`; this
  phase only adds the domain blend.

## Surface after this phase

- `seed_soil` now layers a coarse macro-octave (`MACRO_SEED`, `MACRO_PASSES`,
  `MACRO_FLOOR`) multiplicatively over the existing fine soil patches; `soil`
  and `capacity` stay in [0, 1] / [0, MAX_GRASS].
- `capacity(index) = water_prox · soil · MAX_GRASS` is unchanged in shape.
- No river, water, browse, or elk behavior is changed.
