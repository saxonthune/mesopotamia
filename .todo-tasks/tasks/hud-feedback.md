# HUD Feedback — per-group live stats, drive-mix, pull-share, trend arrows

## Motivation

Tweaking a slider should *visibly* move something. Today the controls are mostly mute: you
drag a weight and nothing tells you what it did. Now that the green wave makes the natural
drives matter and the pull penalty makes reliance on the migration pull legible, the most
diagnostic thing we can show is **how the herd is actually moving** (the drive mix) and **how
much it's leaning on the magic pull** (`score.pull_share`, added by the pull-penalty phase).
This phase surfaces that, plus simple up/down trend arrows so every tweak has feedback.

Scope is deliberately bounded to three concrete, well-anchored additions in `src/ui.rs`, all
reading signals that already exist (`DriveSamples`, `Score`, `History`). No new graph
windows, no sparklines.

## Do NOT

- **Do NOT add new graph/plot windows, sparklines, or a new `Graph` enum variant.** Reuse the
  existing render primitives (`RichText` labels, `allocate_painter` bars like
  `render_difficulty`). Keep it compact.
- **Do NOT change simulation behaviour.** This is a read-only HUD phase — `src/ui.rs` only
  (plus, if unavoidable, threading an existing `Res` through a function signature). No edits
  to movement, score, grid, or params logic.
- **Do NOT recompute drive magnitudes from scratch** — `DriveSamples.per_slot[i]` already
  holds per-slot `sep/coh/grass/social/migration` means and a `migration_share()`. Average
  the slots; don't re-derive from elk queries.
- **Do NOT invent a new pull-share signal** — read `score.pull_share` (provided by the
  pull-penalty phase). Do not compute your own from `DriveSamples` for the pull readout; the
  smoothed `score.pull_share` is the canonical one the score uses.
- **Do NOT block on missing data** — empty herd / empty history must render a calm
  placeholder, never panic or divide by zero.
- `just test-fast` in the loop; one `test-all` at the end.

## Plan

Anchors (current line numbers, approximate — locate by name):
- `render_score_gauge(ui, score)` — ui.rs ~204; called by `graphs_bar` (~401) which already
  has `history: Res<History>`.
- `behaviour_tab(ui, p, bite_ratio, cross_ratio)` — ui.rs ~1081; the drive-weight + migration
  sliders. Called from `control_panel` (~545), which already has
  `drive_samples: Res<DriveSamples>`.
- `DRIVE_COLORS` / `Drives::contributions` order is **sep, coh, grass, social, migration** —
  match it everywhere.

### 1. Drive-mix readout (pure `drive_mix` + small widget)

Pure function (put near the other pure UI helpers, e.g. beside `world_viewport`, or in a tiny
`mod` — keep it unit-testable):

```rust
/// Herd-mean normalised drive mix, in contributions() order: [sep, coh, grass, social,
/// migration]. Averages each component's magnitude over slots with count > 0, then
/// normalises to sum 1. All-zero (idle herd) returns [0; 5] — callers show a placeholder.
pub fn drive_mix(samples: &DriveSamples) -> [f32; 5] { ... }
```

Render a compact horizontal stacked bar (or five `xx%` chips) labelled by drive, coloured by
`DRIVE_COLORS`, at the **top of `behaviour_tab`** above "drive weights". This shows, live,
what fraction of the herd's movement each weight is currently buying — so dragging a slider
visibly reshapes the bar. `migration`'s segment is the herd's pull reliance made visible.

### 2. Pull-share readout near the migration slider

Add a small readout right under the `migration ÷ crossing cost` slider in `behaviour_tab`:
`magic pull  NN%`, coloured green (low) → orange → red (high) — reuse the
`difficulty_color` ramp or a parallel one — reading `score.pull_share`. Hover text: relying
on the pull caps your score (the pull penalty), so a high reading means you're leaving points
on the table.

This requires threading `Score` into `behaviour_tab`. `control_panel` does **not** currently
take `Score` — add `score: Res<Score>` to `control_panel` and pass `&score` down through to
`behaviour_tab`. (Confirm `behaviour_tab`'s other callers, if any, are updated.)

### 3. Trend arrows (pure `trend` + a vitals line)

Pure function:

```rust
enum Trend { Up, Down, Flat }
/// Direction of a series over the last `lookback` samples: compares the latest value to the
/// one `lookback` back, Flat if the absolute change is within `eps` or history is too short.
fn trend(series: &VecDeque<f32>, lookback: usize, eps: f32) -> Trend { ... }
fn trend_arrow(t: Trend) -> &'static str { /* "↑" / "↓" / "→" */ }
```

Add a compact **herd vitals** line inside `render_score_gauge` (below the score number,
above or below the difficulty section): `pop N ↑   energy 0.NN →   pull NN% ↓`, each value
followed by its `trend_arrow`, computed from `History` (`population`, `avg_energy`, and the
per-slot `migration_share` averaged — or `score.pull_share` for pull). This means threading
`history: &History` into `render_score_gauge` (caller `graphs_bar` already has it) and
passing `score` (already passed). Pick a `lookback` of a few hundred samples and a small
`eps` so the arrows are stable, not jittering every frame.

### 4. Tests (verification skill — pure, name the breaking input)

- `drive_mix`: **sums to 1** when any component is non-zero; **all-zero → [0;5]**;
  **order** is sep,coh,grass,social,migration (a sample with only `grass` set yields the mix
  concentrated in index 2); averaging across slots is correct (two slots → mean).
- `trend`: **Up** when latest > value `lookback` back by more than `eps`; **Down** symmetric;
  **Flat** within `eps`; **Flat** when the series is shorter than `lookback` (empty-safe).

## Files to Modify

- `src/ui.rs` — `drive_mix` + widget at top of `behaviour_tab`; pull-share readout under the
  migration slider; `trend`/`trend_arrow` + vitals line in `render_score_gauge`; thread
  `Score` through `control_panel` → `behaviour_tab` and `&History` into `render_score_gauge`;
  tests for `drive_mix` and `trend`.

## Verification

```bash
just test-fast
```

Lib tests pass, including `drive_mix` and `trend`. The binary builds (the UI compiles).
Then once:

```bash
just test-all
```

## Out of Scope

- Sparklines / mini-plots next to sliders; new graph windows; restyling the existing graphs.
- Per-slider (vs per-group) statistics — group-level readouts only.
- A delta-flash animation on slider change — nice-to-have, not now.
- Any change to what `History` records (its series are sufficient).

## Notes

- This phase triages against the **pull-penalty** surface: `score.pull_share: f32` exists and
  is the canonical smoothed pull reliance. If for any reason that field is absent, fall back
  to the herd-mean of `DriveSamples` `migration_share()` and note it — but it should be there.
- Keep the widgets quiet and small; this is feedback, not a dashboard. Match the existing
  `RichText` sizing/weak-label idiom (see `render_difficulty`).
- `egui` arrow glyphs: ensure the chosen arrows render in the installed font
  (`install_icon_font`); if not, use ASCII `^ v ->` or simple triangles.

## Surface after this phase

- `ui::drive_mix(&DriveSamples) -> [f32; 5]` — pure, normalised herd drive mix in
  contributions() order.
- `ui::trend(&VecDeque<f32>, lookback, eps) -> Trend` + `trend_arrow(Trend)` — pure trend
  helpers.
- `behaviour_tab` shows a live drive-mix bar and a `magic pull NN%` readout; `control_panel`
  and `behaviour_tab` now take `Score`.
- `render_score_gauge` shows a herd-vitals line with trend arrows and now takes `&History`.
- No simulation behaviour changed; all additions are read-only HUD.
