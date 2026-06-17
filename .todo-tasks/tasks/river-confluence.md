# River confluence (merge two mains into one)

## Motivation

The main rivers run top→bottom in parallel and never interact. This phase adds a
confluence: one river's channel converges into another's downstream and they
continue as a single channel, producing a landmark and a natural crossing
chokepoint for the migrating elk. It builds on the per-river variety from the
`river-variety` phase.

This phase triages against the **Surface after this phase** block of
`river-variety` (which has merged to trunk before this runs), not live code.
Relevant guarantees from that surface:
- `RiverSpec` has fields `count`, `drift`, `bendiness`, `drift_spread`,
  `bendiness_spread`, `radius`, `core`, plus lake/oxbow/tributary/ford fields.
- `generate_river_inner(grid, spec) -> (mains, tribs)`; `mains.len() == spec.count`;
  each main carved with its own per-river drift/penalty derived from `RIVER_SEED`
  + index; **currently every main runs row 0 → row height-1**.
- `carve(grid, cost, start, goal, bias)` does anisotropic least-cost; tributaries
  already carve to a confluence cell sampled from a main centerline.

## Do NOT

- Do NOT build a delta / braided fan-out — the chosen topology is "merge two into
  one". Exactly one confluence pair by default.
- Do NOT change the per-river spread or lake logic from the previous phase.
- Do NOT leave the merged child river also reaching the bottom edge — once it
  joins, only the parent channel continues to the exit.
- Do NOT wall off the +x migration axis: the merged channel plus fords must keep
  the map crossable (the macro crossing test is the canary).

## Plan

### 1. Declarative confluence pairs

Add a `RiverSpec` field `confluence_pairs: Vec<(usize, usize)>`, default
`vec![(1, 0)]` — each `(child, parent)` means the `child` river merges into the
`parent` river. Require `parent < child` so the parent is already carved when the
child is carved (mains are carved in index order).

### 2. Carve the child to a confluence on the parent

In `generate_river_inner`'s main loop, when river `i` is the `child` of a
confluence pair:
- Instead of setting its goal to the bottom-edge exit, set the goal to a cell on
  the **parent's already-carved centerline** in the lower portion of the map
  (e.g. sampled from the parent centerline below ~60% of `height`, mirroring the
  tributary confluence selection but as a full-width main, using `spec.radius`).
- Carve with the river's per-river heading bias (it is still a main channel), and
  `rasterize` at `spec.radius` / `spec.core` / depth `1.0`.
- The child's centerline therefore ends (`cl[0]`) at the confluence cell on the
  parent, not at the bottom edge. Below the confluence, only the parent channel
  exists — do not extend the child past the join.

Keep `mains.len() == spec.count` (the child is still a main, just a shorter one
that terminates at the join). Non-child rivers are unchanged.

### 3. Update the topology test

`rivers_flow_top_to_bottom` currently asserts every main exits at row
`height-1`. Update it to read `spec.confluence_pairs`:
- Non-child mains: still assert entry row 0 and exit row `height-1` with rightward
  drift.
- Child mains: assert entry row 0, and that the exit cell (`cl[0]`) lies on the
  parent's centerline (set-membership, like `tributaries_join_a_main_channel`) —
  i.e. the child reaches the parent rather than the bottom edge.

## Files to Modify

- `src/river.rs` — `RiverSpec.confluence_pairs` + default, child-goal routing in
  the main loop, confluence rasterize.
- `src/river.rs` tests — update `rivers_flow_top_to_bottom` for child rivers; add
  `confluence_child_joins_parent` (the child's exit cell is on the parent
  centerline). Keep `generation_is_deterministic` green.

## Verification

```bash
cargo test --lib
cargo test --test macro_sim
cargo build --bin mesopotamia
```

## Out of Scope

- Deltas / distributaries.
- Regional grass fertility (phase C).
- Elk/movement changes.

## Notes

- Determinism: if you sample the confluence position, use the existing seeded
  `rng` (or a per-pair child RNG) — never the clock.
- If the crossing test fails because the merged channel concentrates water on the
  migration axis, ensure fords still stamp across the merged channel; adjust ford
  spacing/lake placement before touching the test threshold.

## Surface after this phase

- `RiverSpec` gains `confluence_pairs: Vec<(usize, usize)>` (default `vec![(1,0)]`);
  all prior fields unchanged.
- `generate_river_inner` signature/return unchanged; `mains.len() == spec.count`.
- For each `(child, parent)` in `confluence_pairs`, the child main's centerline
  terminates at a confluence cell on the parent's centerline (lower map portion);
  all non-child mains still run row 0 → row height-1.
- Lakes, oxbows, tributaries, fords, per-river spread, `compute_water_prox` all
  still exist and behave as in the previous phase.
