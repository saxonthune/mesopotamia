# Metric Registry + Distribution Histograms

## Motivation

Rung 3 (doc02.03): a mean erases the structure that *is* the emergent phenomenon — a herd split
between sated and starving has the same mean energy as a uniformly healthy one. The legible view is
the live *distribution*. Separately, the codebase keeps three near-duplicates of "named pure scalar
over state": `history.rs` series, `DriveSamples`, and the balance-sweep objective (doc02.02,
`sim_harness.rs`). A single registry of named pure extractors unifies them.

## Do NOT

- Do NOT change what the balance sweep optimizes (doc02.02 semantics: survival / journey /
  migration-share). This phase only routes the existing extractors through a registry — same numbers.
- Do NOT capture `World`/`Query` inside an extractor where a plain `&Elk` (or a slice) suffices —
  extractors stay pure so they run in the UI and headless alike.
- Do NOT add overlays, decision records, event logs, or probes.

## Plan

### 1. Metric registry (new `src/metrics.rs`)

Define a per-agent metric as a named pure extractor and a small registry:

```rust
pub struct Metric { pub name: &'static str, pub extract: fn(&crate::elk::Elk) -> f32 }
pub const ELK_METRICS: &[Metric] = &[
    Metric { name: "energy", extract: |e| e.energy },
    // add as needed
];
```

Unit-test that `energy` returns `Elk.energy`. Keep it data-only so the sweep and UI both enumerate it.

### 2. Histogram UI (`src/ui.rs`)

Add a histogram view driven by `ELK_METRICS`: bin the selected metric across all live elk into a
fixed number of buckets and draw a bar chart (egui bars, or a `Plot` with a bar series). Surface it
as a new overview toggle (mirror `Graph`/`graphs_bar`) or a section in the Herds/Sliders tab. Energy
first; the metric is selectable from the registry.

### 3. Reconcile existing consumers (`src/history.rs`, `src/sim_harness.rs`)

Where cheap and non-behavioural, have `history.rs`'s mean-energy sampling and the histogram read the
same `ELK_METRICS` extractor rather than inlining `elk.energy`. Do NOT rewrite the sweep's
objective; optionally note in a comment where a future unification with `ELK_METRICS` would land.

## Files to Modify

- `src/metrics.rs` (new) — `Metric`, `ELK_METRICS`, unit tests; register module in `src/lib.rs`.
- `src/ui.rs` — histogram view + toggle.
- `src/history.rs` — route mean-energy through the registry extractor (optional, if non-behavioural).

## Verification

```bash
just test-fast
cargo build
```

## Out of Scope

- Decision record, overlays, event log, conservation ledger, probes.
- Changing balance-objective semantics (doc02.02) — plumbing only.

## Notes

- Chain `sim-observability`, phase 3 of 6. Largely independent of phases 1–2; safe to triage against
  current `history.rs`/`sim_harness.rs` (the merge left `history.rs` unchanged).
- `Elk` lives in `src/elk/components.rs` and is exported as `crate::elk::Elk`. Confirm field names
  (`energy`, `cell`, `slot`, `code`).
- Patterns: doc02.01 (pure tested functions), doc02.02 (the metric/sweep definitions).

## Surface after this phase

- `src/metrics.rs` with `pub struct Metric { name, extract: fn(&Elk)->f32 }` and a
  `pub const ELK_METRICS: &[Metric]` (energy at minimum), exported as `crate::metrics`.
- A histogram UI view over `ELK_METRICS`, toggleable from the UI.
- Simulation behaviour and the balance sweep semantics are unchanged.
