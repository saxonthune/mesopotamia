# Spatial Event Log

## Motivation

Rung 4 (doc02.03): a counter records *how many*; it discards the *where* and the *state*, which are
the diagnostic payload. `metabolize` (`src/elk/metabolism.rs`) despawns a starving elk and
increments `Cohort.deaths` — the count survives, the context dies. Deaths clustered tight against a
barrier with forage visible beyond it localize a bug to a coordinate in one glance;
cluster-against-a-barrier is a signature the count can never show.

## Do NOT

- Do NOT make events affect the simulation — they are observational only.
- Do NOT persist events to disk; this is an in-memory ring buffer.
- Do NOT wire every possible event now — mortality (starvation) only; leave a clear extension point.
- Do NOT require `LastDecision` (phase 1) to be present — capture it when available, degrade when not.

## Plan

### 1. Event record + ring-buffer resource (new `src/events.rs`)

```rust
pub enum EventKind { Starved }
pub struct Event { pub tick: u64, pub cell: usize, pub kind: EventKind, pub energy: f32 }
#[derive(Resource, Default)] pub struct EventLog { pub recent: std::collections::VecDeque<Event> }
```

Bound `recent` to a fixed cap (mirror `history.rs`'s `WINDOW`). Add a `push` that pops the front when
full. Register the resource via a small plugin or in an existing plugin's `build`.

### 2. Emit on mortality (`src/elk/metabolism.rs`)

At the despawn site in `metabolize` where `energy <= 0`, push a `Starved` event capturing the elk's
`cell` and `energy` (and tick — read the `Sim` tick / a frame counter; if none exists, use the
fixed-update count already available, or add a tick resource). If a `LastDecision` is queryable for
that elk, capture a compact summary into the event (optional field) — but do not add it as a hard
dependency.

### 3. Death-map overlay + recent-events list (`src/ui.rs` + overlay layer)

Reuse phase 2's overlay infrastructure: add a `DeathSites` overlay variant that draws a marker at
each recent event's `cell`. Add a small egui list (Herds tab or a new section) of the most recent
events: tick, cell, energy. Toggleable; off by default.

## Files to Modify

- `src/events.rs` (new) — `Event`/`EventKind`/`EventLog` + plugin; register in `src/lib.rs`/binaries.
- `src/elk/metabolism.rs` — push a `Starved` event at the despawn site.
- `src/ui.rs` — recent-events list + `DeathSites` overlay toggle.
- overlay layer from phase 2 — render death markers.

## Verification

```bash
just test-fast
cargo build
```

## Out of Scope

- Spawn / graze / crossing events (leave the `EventKind` enum extensible).
- The introspection panel and histograms (other phases).

## Notes

- Chain `sim-observability`, phase 4 of 6. Depends on phase 1 (`LastDecision`, optional context) and
  phase 2 (overlay infra for the death-map). **Triage against phases 1 & 2 Surface blocks**, not a
  re-derivation. Degrade gracefully if `LastDecision` is absent.
- `metabolize` runs in `FixedUpdate` gated on `Sim::Running`; the event push belongs there.
- A tick source: check whether a frame/tick counter exists; if not, the simplest is a
  `#[derive(Resource)] struct Tick(u64)` incremented in a `FixedUpdate` system, or reuse
  `Spawner.elapsed` (see `sim_harness::spawner_elapsed`).
- Patterns: doc02.01, doc02.03 (the cluster-against-a-barrier signature).

## Surface after this phase

- `src/events.rs` with `Event`, `EventKind` (extensible; `Starved` present), and an `EventLog`
  ring-buffer `Resource`, exported as `crate::events`.
- Mortality events emitted from `metabolize`, carrying cell + energy (+ optional decision context).
- A `DeathSites` overlay variant + a recent-events UI list, both toggleable, observational only.
