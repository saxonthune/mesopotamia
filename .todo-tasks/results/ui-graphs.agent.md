# Agent Result: ui-graphs

date: 2026-06-16T22:07:09-04:00
session: completed
verification: passed
commits: 1
branch: chain-demo-readouts_claude_ui-graphs
surface deviations: declared
session id: 0bebb227-84e4-4ab3-8a1b-a4ab60de16c9


## Summary

All done. Here's a summary of what was implemented:

---

## Implementation Summary

Implemented all four steps of the `ui-graphs` plan:

1. **`Cargo.toml`** — Added `egui_plot = "0.34"` (not 0.33 — see Notes).

2. **`src/history.rs`** (new) — `History` resource with `VecDeque<f32>` ring buffers for population, avg_energy, and per-slot migration_share (capped at 600 samples). The `sample_history` system counts alive elk directly via `Query<&Elk>`, reads `DriveSamples` for each slot, and pushes values each `Update` tick.

3. **`src/lib.rs`** — Added `pub mod history;`.

4. **`src/ui.rs`** — Added `PlotSpec<'a>` and `PieSpec<'a>` structs, `Item::Plot` and `Item::Pie` variants, `render_plot` (egui_plot line graph) and `render_pie` (triangle-fan painter, ~30 lines) helpers, wired `History` and `DriveSamples` through `control_panel` → `herds_view` → `herd_details`. The `herd_details` pane now shows a drive-split pie (5 drives × named colors) and line graphs for population and per-slot migration share.

5. **`src/bin/demo1.rs`** — Added `#[path = "../history.rs"] mod history;` to satisfy the `crate::history` references that `ui.rs` needs when compiled as part of the binary's own crate namespace.

All verification commands pass: `cargo build --bin mesopotamia`, `cargo build --bin demo1`, `cargo test` (3 tests), `bash smoke.sh --no-run`.

---

## Notes

- **egui_plot version deviation**: The plan specified `egui_plot = "0.33"`, but `egui_plot 0.33` actually depends on `egui 0.32`, not `0.33`. Since `bevy_egui 0.39.1` uses `egui 0.33.3`, the two would create conflicting egui versions. Used `egui_plot = "0.34"` (resolves to `0.34.1`) which aligns with egui 0.33.x. The plan itself anticipated this: "if a 0.33 `egui_plot` is unavailable, pin the closest matching minor and note it."
- **demo1.rs modification**: The plan listed only `ui.rs`, `lib.rs`, `Cargo.toml`, and `history.rs` as files to modify. `demo1.rs` also needed `#[path = "../history.rs"] mod history;` because it uses `#[path]`-based module inclusion rather than the lib crate. This is an extension of the pattern already in `demo1.rs`, not a design change.
- **`Line::new` API**: `egui_plot 0.34` takes `Line::new(name, points)` (name first), consistent with what the lock file shows.

---

## Surface Deviations

None. All declared Surface items are present:
- `mesopotamia::history::History` resource + `sample_history` system ✓
- `Item::Plot`/`Item::Pie` variants in `src/ui.rs` ✓
- `egui_plot` line-graph render arm (`render_plot`) ✓
- `Painter` pie helper (`render_pie`) ✓
- `herd_details` showing drive-split pie + population/share plots ✓
- `egui_plot` dependency in `Cargo.toml` (0.34, not 0.33 — documented above) ✓

## Commits

```
3c70d83 feat: ui-graphs — History ring buffer, Item::Plot/Pie, drive-split readouts
```

## Build & Test Output (last 30 lines)

```

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s


running 1 test
.
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.70s


running 3 tests
...
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 48.02s


running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

unit tests: PASS

[1m=== build (cargo build) ===[0m
build demo1: PASS
build mesopotamia: PASS

[1m=== runtime launch ===[0m
skipped (--no-run)

[1;32mALL SMOKE CHECKS PASSED[0m
```
