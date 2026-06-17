# River confluence (merge two mains into one)

## Motivation

The main rivers run top→bottom in parallel and never interact. This phase adds a
confluence: one river's channel converges into another's downstream and they
continue as a single channel, producing a landmark and a natural crossing
chokepoint for the migrating elk.

This is phase 2 of a 2-phase chain (regional-fertility → river-confluence). It is
independent of the fertility phase (different files), and `src/river/` is exactly
as it is on trunk. `river.rs` was refactored into a `river/` module — triage
against that live layout:
- `src/river/spec.rs` — `RiverSpec` (incl. `count`, `drift`, `radius`, `core`,
  `drift_spread`, lake/oxbow/tributary/ford fields), `Heading`, the seed/penalty
  constants.
- `src/river/mod.rs` — `generate_river_inner(grid, spec) -> (mains, tribs)`, the
  main-river carve loop, the integration tests (incl. `rivers_flow_top_to_bottom`,
  `tributaries_join_a_main_channel`). `carve` and `rasterize` are already imported
  here from the submodules.
- `src/river/cost.rs` — `carve(grid, cost, start, goal, bias)` (anisotropic
  least-cost; `pub(super)`).
- `src/river/raster.rs` — `rasterize` and ford stamping (`pub(super)`).
- `src/river/features.rs` — oxbows, lakes, tributaries (tributaries already carve
  to a confluence cell sampled from a main centerline — the model to mirror).

Currently every main runs row 0 → row height-1.

## Agent execution notes (READ FIRST)

- **Run every cargo/build command in the FOREGROUND and let it block to
  completion.** Do NOT background it and poll with `sleep`/`tail` — this sandbox
  BLOCKS that pattern and you have no polling tool, so you will strand yourself
  and lose your work. The Verification commands use a pre-warmed shared target
  dir, so compiles take seconds.
- Commit after each logical unit of work; verify with `git log --oneline -3`
  before finishing.

## Do NOT

- Do NOT build a delta / braided fan-out — the chosen topology is "merge two into
  one". Exactly one confluence pair by default.
- Do NOT leave the merged child river also reaching the bottom edge — once it
  joins, only the parent channel continues to the exit.
- Do NOT wall off the +x migration axis: the merged channel plus fords must keep
  the map crossable. `macro_sim::herds_reach_the_far_edge` is the canary — it
  MUST stay green.
- Do NOT touch `src/grid.rs` fertility work or elk/movement.

## Plan

### 1. Declarative confluence pairs

In `src/river/spec.rs`, add a `RiverSpec` field
`confluence_pairs: Vec<(usize, usize)>`, default `vec![(1, 0)]` — each
`(child, parent)` means the `child` river merges into the `parent`. Require
`parent < child` so the parent is already carved when the child is carved (mains
are carved in index order).

### 2. Carve the child to a confluence on the parent

In `generate_river_inner`'s main loop (`src/river/mod.rs`), when river `i` is the
`child` of a confluence pair:
- Instead of the bottom-edge exit, set the carve goal to a cell on the parent's
  already-carved centerline (`mains[parent]`) in the lower portion of the map
  (e.g. sampled from the parent centerline below ~60% of `height`, mirroring the
  tributary confluence selection in `features.rs` but as a full-width main using
  `spec.radius`).
- Carve with the river's per-river heading bias (still a main channel) and
  `rasterize` at `spec.radius` / `spec.core` / depth `1.0`.
- The child's centerline therefore ends (`cl[0]`) at the confluence cell on the
  parent, not at the bottom edge. Do not extend the child past the join.

Keep `mains.len() == spec.count` (the child is still a main, just shorter). Use
the existing seeded `rng` (or a per-pair child RNG) for any sampling — never the
clock.

### 3. Update the topology test

`rivers_flow_top_to_bottom` (in `src/river/mod.rs` tests) asserts every main exits
at row `height-1`. Update it to read `spec.confluence_pairs`:
- Non-child mains: still assert entry row 0, exit row `height-1`, rightward drift.
- Child mains: assert entry row 0, and that the exit cell (`cl[0]`) lies on the
  parent's centerline (set-membership, like `tributaries_join_a_main_channel`).
- Add `confluence_child_joins_parent` asserting the child's exit cell is on the
  parent centerline.

## Files to Modify

- `src/river/spec.rs` — `RiverSpec.confluence_pairs` + default.
- `src/river/mod.rs` — child-goal routing in the main loop; update
  `rivers_flow_top_to_bottom`; add `confluence_child_joins_parent`.

## Verification

Run these in the FOREGROUND (see Agent execution notes). `CARGO_TARGET_DIR` points
at a pre-warmed shared target so compiles take seconds.

```bash
CARGO_TARGET_DIR=/home/saxon/.cache/mesopotamia-shared-target cargo test --lib
CARGO_TARGET_DIR=/home/saxon/.cache/mesopotamia-shared-target cargo test --test macro_sim
```

## Out of Scope

- Deltas / distributaries.
- Regional grass fertility (phase 1).
- Elk/movement changes.

## Notes

- If the crossing test fails because the merged channel concentrates water on the
  migration axis, ensure fords still stamp across the merged channel (ford
  stamping iterates `mains`, and the child is still in `mains`); adjust ford
  spacing before touching the test threshold.
- Determinism: `generation_is_deterministic` must stay green.

## Surface after this phase

- `RiverSpec` (in `src/river/spec.rs`) gains `confluence_pairs: Vec<(usize, usize)>`
  (default `vec![(1,0)]`); all prior fields unchanged.
- `generate_river_inner` signature/return unchanged; `mains.len() == spec.count`.
- For each `(child, parent)`, the child main's centerline terminates at a
  confluence cell on the parent's centerline; all non-child mains still run
  row 0 → row height-1.
- Lakes, oxbows, tributaries, fords, per-river spread, the module split, and
  `compute_water_prox` all still behave as on trunk.
