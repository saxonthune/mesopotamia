# Drive sampling resource (per-tick migration share)

## Motivation

Both the balancing sweep and the UI pie/graphs need the *running* sim to expose each herd's
instantaneous drive breakdown — `migration_share` and the per-drive magnitudes (doc02.02). The
sim already computes these per elk inside `herd_move` (via `combine_drives`, from the
`drive-decomposition-metric` task) but discards them. This task records them into a resource so
downstream consumers read them instead of recomputing the neighbour math. It is a single small
write in one place so the sweep and the UI-graphs task never both edit `movement.rs` (they stay
parallel).

Follows doc02.01: one concept (sampling), minimal surface. Read doc02.02 for the metric defs.

## Do NOT

- **Do NOT change movement behaviour.** This only *reads* the `Drives` already computed in
  `herd_move` and writes aggregates to a resource. The step pick is untouched.
- **Do NOT** add a separate system that recomputes drives — that would duplicate the neighbour
  loop and could drift. Aggregate inside `herd_move` where `Drives` already exist.
- **Do NOT** build a time-series ring buffer here — this resource holds only the *latest* tick's
  per-slot aggregate. Buffering over time is the UI-graphs task's concern.
- **Do NOT** make the resource depend on rendering; it must populate under `MinimalPlugins` so
  the headless sweep reads it.

## Plan

### 1. Define the resource (`src/elk/components.rs`)

```rust
/// Latest-tick mean drive breakdown per pack slot, written by `herd_move` and
/// read by the UI (pie/graphs) and the balancing sweep. Index by `slot`.
#[derive(Resource, Default)]
pub struct DriveSamples {
    /// Mean `Drives` over the elk of each slot this tick (zeroed Drives for an
    /// empty slot). Length == PACK_COUNT.
    pub per_slot: Vec<DriveSample>,
}

#[derive(Default, Clone, Copy)]
pub struct DriveSample {
    pub sep: f32, pub coh: f32, pub grass: f32, pub social: f32, pub migration: f32, // magnitudes
    pub count: u32,
}
```

Store **magnitudes** (not vectors): that is what the pie and the share need, and it keeps the
sample small. Add a `DriveSample::migration_share() -> f32` matching doc02.02 (|migration| over
sum of the five magnitudes, 0 when idle) so consumers don't re-derive it.

### 2. Initialize it (`src/elk/mod.rs`)

`ElkSimPlugin::build` inserts `DriveSamples` sized to `PACK_COUNT`
(`per_slot: vec![DriveSample::default(); PACK_COUNT]`). Re-export `DriveSamples`/`DriveSample`
via the `pub use` block.

### 3. Aggregate inside herd_move (`src/elk/movement.rs`)

`herd_move` gains `mut samples: ResMut<DriveSamples>`. At the start of the decide-and-write
phase, zero a local `[DriveSample; PACK_COUNT]` accumulator. For each elk, after computing
`drives` (already there from `drive-decomposition-metric`), add each contribution's `.length()`
to the elk's slot accumulator and bump `count`. After the loop, write per-slot **means**
(sum/count, or zeroed when count==0) into `samples.per_slot`.

Keep the two-phase snapshot structure intact; this is pure accumulation alongside the existing
write.

### 4. Tests (`src/elk/movement.rs` test module)

- `DriveSample::migration_share` matches the doc02.02 definition on hand-picked magnitudes
  (including the idle → 0 case).
- A small headless-ish check is optional; the share math is the part worth pinning here.

## Files to Modify

- `src/elk/components.rs` — `DriveSamples`, `DriveSample`, `migration_share`.
- `src/elk/mod.rs` — insert + re-export.
- `src/elk/movement.rs` — aggregate into the resource in `herd_move`.

## Verification

```sh
cargo build --bin mesopotamia
cargo build --bin demo1
cargo test
bash smoke.sh --no-run
```

## Out of Scope

- Time-series buffering, graphs, pie (UI-graphs task).
- Reading the resource in the sweep (sweep task).

## Notes

- Depends on the `drive-decomposition-metric` Surface: `Drives` and `combine_drives` exist and
  `herd_move` already builds `drives` per elk. Triage/implement against that Surface.
- Per-slot (pack) granularity matches how the UI selects a herd by `slot` and how the sweep
  averages across packs.

## Surface after this phase

- `mesopotamia::elk::DriveSamples` — `Resource` with `per_slot: Vec<DriveSample>` indexed by
  `slot`, populated every tick by `herd_move`, length `PACK_COUNT`, working under
  `MinimalPlugins` (headless).
- `mesopotamia::elk::DriveSample` — `{ sep, coh, grass, social, migration: f32 (magnitudes), count: u32 }`
  with `migration_share() -> f32` (doc02.02).
- Behaviour: movement unchanged; only a new read-only aggregate is written.
- Negative space: no time-series buffer, no graph widgets, no sweep consumption yet.
