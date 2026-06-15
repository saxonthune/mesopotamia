# Sim / render plugin split (elk)

## Motivation

`ElkPlugin` bundles simulation systems (`FixedUpdate`) with two render-sync systems
(`sync_elk_transform`, `sync_elk_color` on `Update`). This has drifted from `doc03.01.02`
(Structure), which already declares that reflecting elk into sprites belongs to
`render.rs` / `RenderPlugin`. Reconciling toward the doc gives a sim half that loads
**without** rendering — making the macro-state harness genuinely headless instead of
tolerating render systems as no-ops, and matching the project's "structure is the test
affordance" principle.

## Sequencing — runs `--after` the fording chain

`water-as-barrier-fords` edits `elk.rs` by line number (`herd_move` scoring ~line 461,
`ElkParams` ~line 327). Moving systems between plugins shifts those lines and would
conflict. **This task must launch only after `water-as-barrier-fords` has merged.** It also
depends on `macro-state-test-harness` having merged (it relies on `mesopotamia::lib` and the
green harness as a regression gate).

## Do NOT

- **Do NOT move or rename any simulation system.** `herd_move`, `graze`, `digest`,
  `metabolize`, `migrate_pressure`, `spawn_waves`, `cull`, `tally_herds` stay exactly where
  they are, in `elk.rs`, on the same schedules. Only the two **render-sync** systems move.
- **Do NOT change behaviour or tuning.** Pure relocation of rendering concerns.
- **Do NOT break `tests/macro_sim.rs`.** After the split the harness adds `ElkSimPlugin`
  (no render). The three invariants must still pass unchanged.
- **Do NOT reorganise `elk.rs` into submodules** — that's the next task. Keep `elk.rs` one
  file; only the plugin wiring and two systems' homes change.
- **Do NOT touch `river.rs`** (procgen-owned).

## Plan

### 1. Split `ElkPlugin` into a sim plugin and a render plugin

In `elk.rs`, rename the sim half to `ElkSimPlugin` and have its `build` register only the
`FixedUpdate` sim systems plus `tally_herds` (a sim bookkeeping system, keep it sim-side).
The two render-sync systems (`sync_elk_transform`, `sync_elk_color`) move out.

Per `doc03.01.02`, elk→sprite rendering belongs in `render.rs`. Move `sync_elk_transform`
and `sync_elk_color` (and the private `cell_pos` helper they need, plus `elk_color` if only
they use it — check call sites; `spawn_pack` also calls `elk_color`, so keep `elk_color`
`pub(crate)` in `elk.rs` and import it into `render.rs`) into `render.rs`, registered by
`RenderPlugin` on `Update`.

### 2. Update composition roots

`main.rs` and `demo1.rs` currently add `ElkPlugin`. Replace with `ElkSimPlugin`, and ensure
`RenderPlugin` now owns the elk-sync systems. Both binaries render elk exactly as before.

### 3. Reconcile `doc03.01.02` in place

Rewrite the relevant lines of `.carta/03-milestones/01-grazers/02-structure.md` to describe
the sim/render split (e.g. `ElkSimPlugin` owns elk simulation; `RenderPlugin` owns elk→sprite
sync). Present-tense declarative prose; rewrite in place, no status/changelog sections (per
the docs-development conventions).

## Files to Modify

- `src/elk.rs` — `ElkPlugin` → `ElkSimPlugin` (sim systems only); remove the two render
  systems and `cell_pos`; make `elk_color` `pub(crate)`.
- `src/render.rs` — gain `sync_elk_transform`, `sync_elk_color`, `cell_pos`; `RenderPlugin`
  registers them on `Update`; import `elk_color`.
- `src/main.rs` — `ElkSimPlugin` in place of `ElkPlugin`.
- `src/bin/demo1.rs` — same swap.
- `tests/macro_sim.rs` — `ElkSimPlugin` in place of `ElkPlugin` in `headless()`.
- `.carta/03-milestones/01-grazers/02-structure.md` — describe the split.

## Verification

```sh
cargo test
cargo test --test macro_sim
cargo build --bin mesopotamia
cargo build --bin demo1
bash smoke.sh
```

## Out of Scope

- `elk.rs` module split into `elk/` submodules (next task).
- Any movement/grazing logic change.

## Notes

- Triage against the fording chain's `## Surface after this phase` for `elk.rs` line
  positions if it hasn't merged at triage time — but execution waits until it has.

## Surface after this phase

- `ElkSimPlugin` (renamed from `ElkPlugin`) registers only simulation systems and loads with
  no rendering dependency. `tests/macro_sim.rs` uses it.
- `RenderPlugin` owns `sync_elk_transform` + `sync_elk_color`.
- `elk_color` is `pub(crate)` in `elk.rs`.
- `doc03.01.02` describes the sim/render split.
- Negative space: `elk.rs` is still a single file (no `elk/` submodule directory yet).
