# Green Wave, Fixed — steer by forage freshness so the natural drive crosses

## Motivation

The shipped green wave does not work. Harness sweeps (`tests/herd_shape.rs`,
`evaluate_bundle`) show that at **zero migration pull**, no wave configuration — any
speed, wavelength, or strength — moves the herd's mass east; every setting ties or
*underperforms* wave-off, and strong settings cut survival. The migration pull remains
the only thing that crosses, so the score's optimum is still the degenerate "max pull +
max scarcity", and the three-stage game arc (`doc02.03`) has no skill path in stage 2.

**Root cause:** the herd steers by *biomass* (`grass_gradient` climbs `grid.forage` =
grass + shrubs), but the wave concentrates the most biomass in the **mature zone behind
the crest** (grass that has grown longest), so following biomass pulls the herd
*backward*. The real green wave works because animals track forage **quality**, which
peaks at the fresh green-up *front*, not at peak biomass.

This pass makes that real: a per-cell **freshness** signal (a decaying accumulator of
recent grass regrowth) that peaks where the wave is actively greening — the eastward-
moving front — and blends into the grass drive. Following freshness *is* migrating east
on the natural drive. Plus it re-keys the **pull penalty** so leaning on the pull
actually costs score, completing the loop the penalty was meant to close.

## Do NOT

- **Do NOT use a biomass "quality hump"** (quality peaking at mid grass/cap). A herd that
  grazes a mature cell down to mid-level would read it as high quality and camp its own
  grazed patch — re-creating the circling bug. Freshness must come from *regrowth*, not
  from the current biomass level.
- **Do NOT remove the migration pull, the green-wave crest, or senescence.** The crest is
  what produces the eastward regrowth that freshness captures; the pull stays a (now
  properly penalized) crutch. Tune senescence down if it starves, but keep the knob.
- **Do NOT break the identity switches.** `freshness_weight == 0` ⇒ the grass drive is
  exactly today's biomass gradient. `GreenWave.strength == 0` ⇒ no wave, no freshness
  growth. Both are required tests.
- **Do NOT touch the shrub field** — freshness is grass-only; shrubs stay the static dry
  reward in `forage()`.
- **Do NOT re-tune or rewrite the preset table** (`src/elk/presets.rs`). Finding the new
  preset values is a separate interactive sweep after this merges. Leave `PRESETS` alone.
- **Do NOT run `cargo test` / `just test-all` in the loop** (it has frozen the machine) —
  `just test-fast` only; one `test-all` at the very end.

## Plan

### 1. Freshness field (`src/grid.rs` + `src/field.rs`)

Add `pub freshness: Vec<f32>` to `Grid`, sized like `grass`, init to 0. Freshness rises
where grass *grew* this tick and decays everywhere:

In `field.rs`, a pure step (tested):
```rust
/// Next freshness field: each cell decays toward 0 at `decay`, then gains `gain ×`
/// this tick's positive grass growth. `grew[i]` is `max(next_grass[i] − prev_grass[i], 0)`
/// — only regrowth raises freshness, never grazing (which lowers grass). Clamped ≥ 0.
pub fn step_freshness(freshness: &[f32], grew: &[f32], decay: f32, gain: f32) -> Vec<f32>
```

In the `growth` system (`grid.rs`): after computing `next` grass via `wave_grow`, compute
`grew[i] = (next[i] − grid.grass[i]).max(0.0)`, then
`grid.freshness = field::step_freshness(&grid.freshness, &grew, FRESH_DECAY, FRESH_GAIN)`.
Add `FRESH_DECAY` / `FRESH_GAIN` constants (a slow decay so a fresh band persists a while
as the crest moves through; tune in the harness). At `GreenWave.strength == 0` the wave
adds no extra regrowth, so freshness stays near the baseline intrinsic regrowth level —
effectively flat — preserving the off-switch.

`Grid::new` and any grid reset must initialise `freshness`. Add a `pub fn freshness(i)`
accessor mirroring `grass(i)`.

### 2. Steer by a freshness-weighted attractiveness (`src/elk/movement.rs`, `components.rs`)

Add `freshness_weight: f32` to `ElkParams` (default 0.0 → identity; the harness will find
the live value). Change `grass_gradient` to climb **attractiveness** rather than raw
forage:
```
attract(n) = grid.forage(n) + params.freshness_weight * grid.freshness(n)
```
i.e. replace the `grid.forage(n)` term in the gradient sum with `attract(n)`. Keep the
shrub contribution inside `forage()` untouched. This is the whole behavioural change: the
grass drive now points up-current toward the fresh green-up front, which advances east, so
the existing drive carries the herd with the wave — no new drive vector, no new pull.

Tests (verification skill — pure, name the breaking input):
- `step_freshness`: rises where `grew > 0`; **does not rise where `grew == 0`** (grazing
  alone never freshens — the anti-camping property); decays toward 0 over ticks with no
  growth; clamped ≥ 0.
- `grass_gradient` with `freshness_weight > 0` points toward a high-freshness neighbour
  even when a *higher-biomass* neighbour sits the other way (the core fix: fresh beats
  mature). With `freshness_weight == 0`, gradient is identical to today (identity).

### 3. Re-key the pull penalty off the dialed pull (`src/elk/score.rs`)

`pull_factor` currently takes the residual-suppressed `pull_share`, which never climbs
enough to bite. Re-key the *score's* penalty to the **dialed** pull (`cross_ratio`, which
`update_score` already has via `RatioControls`):
- Add a `CROSS_REF` constant (the pull level at which the penalty saturates — ~the slider
  max, 2.0). Normalise `p = (cross_ratio / CROSS_REF).clamp(0, 1)`.
- In `update_score`, scale payouts by `pull_factor(p, PULL_PENALTY)` (the pure `pull_factor`
  signature/behaviour is unchanged — only its *argument* changes from realized share to
  normalized dialed pull).
- **Keep `score.pull_share` computed and updated** (the smoothed realized share) — the HUD
  drive-mix / "magic pull %" readout still reads it. Only the *score multiplier* switches
  to the dialed pull.

Tests: payout strictly falls as `cross_ratio` rises (the property that was missing);
`cross_ratio == 0` ⇒ factor 1 (a pure-natural crossing is unpenalised); existing
`pull_factor` endpoint/monotonicity/bounds tests stay green.

### 4. Prove it crosses (`tests/herd_shape.rs`)

Add an `#[ignore]` diagnostic, `green_wave_quality_crosses`, using `evaluate_bundle` at
**zero pull** (`cross_ratio: 0.0`) on the real worldgen map:
- wave **off** (`strength 0`, `freshness_weight 0`) — the baseline (~centroid 11–16 today).
- wave **on** + `freshness_weight` tuned — the fix.
- Print both `PresetOutcome` rows and the centroid/max_col advantage.

**Success criterion (the point of the whole task):** wave-on at zero pull moves the herd
meaningfully further east than wave-off — target **centroid_col at least ~2× wave-off and
max_col well past it** (loose, seed-noisy — assert a generous floor, e.g. wave-on
`max_col > wave_off.max_col + 60`, and print). Tune `freshness_weight`, `FRESH_DECAY`/
`FRESH_GAIN`, and the `GreenWave` speed/strength until this holds. **If, after honest
tuning, the natural drive still will not cross, do not fudge the threshold — report the
best numbers reached and what was tried** in the result; that is a real finding, not a
failure to hide.

### 5. Document

Rewrite the green-wave section in `.rhidoc/03-milestones/01-grazers/01-grazers.md` (find
via `MANIFEST.md`) to describe the fixed mechanism in present tense: the herd steers by
forage *freshness* (recent regrowth), which peaks at the eastward green-up front, so
following fresh forage is the natural migration; the pull is a penalized crutch. No
temporal/MVP prose. Keep `doc02.03` (The Game) consistent — it already describes this
intent, so it should need no change; verify it doesn't contradict the implementation.

## Files to Modify

- `src/grid.rs` — `Grid.freshness` field + accessor + init; `growth` computes `grew` and
  steps freshness; `FRESH_DECAY`/`FRESH_GAIN`.
- `src/field.rs` — `step_freshness` pure fn + tests.
- `src/elk/movement.rs` — `grass_gradient` climbs freshness-weighted attractiveness; tests.
- `src/elk/components.rs` — `ElkParams.freshness_weight` (default 0.0).
- `src/elk/score.rs` — re-key pull penalty to dialed `cross_ratio`; `CROSS_REF`; tests.
- `tests/herd_shape.rs` — `#[ignore]` `green_wave_quality_crosses` diagnostic.
- `.rhidoc/03-milestones/01-grazers/01-grazers.md` — green-wave section rewrite.

## Verification

```bash
just test-fast
```
All lib tests pass, including `step_freshness`, the `grass_gradient` freshness tests, and
the re-keyed `pull_factor` tests; identity tests (`freshness_weight 0`, `strength 0`) hold.
Then once before hand-off:
```bash
just test-all
```
And run the behavioral proof, reporting the numbers:
```bash
cargo test --test herd_shape green_wave_quality_crosses -- --ignored --nocapture
```

## Out of Scope

- **Re-sweeping / updating the preset table** — a separate interactive pass after this
  merges (the harness `evaluate_bundle` finds the new Crossing / High-score values).
- HUD changes — the drive-mix / pull-share readout already exists and still reads
  `score.pull_share`.
- Replacing the green-wave crest primitive — this builds *on* it (freshness captures its
  regrowth); a different primitive (forage frontier) is a later option only if this fails.

## Notes

- The mechanism chain: wave raises the regrowth floor at the crest → cells there green up
  (positive `grew`) → freshness rises at the crest → crest moves east → fresh band moves
  east → freshness-weighted grass drive points east → herd migrates. Each link is testable.
- Watch the **anti-camping** property hardest: freshness must come only from regrowth, so a
  herd grazing a patch down does not make it fresh and does not get pinned. The
  `step_freshness` test on `grew == 0` guards this; if the herd circles in the diagnostic,
  this is the first suspect.
- Senescence (behind the crest) reinforces "don't go back" but can starve; if survival
  craters in the diagnostic, lower the senescence contribution before abandoning the wave.
- Reproducibility not bit-determinism: seed the diagnostic, assert an envelope, print the
  trajectory.

## Surface after this phase

- `Grid.freshness: Vec<f32>` + `Grid::freshness(i)` accessor; initialised on construction;
  stepped each tick by `growth`. `field::step_freshness(freshness, grew, decay, gain)` pure.
- `ElkParams.freshness_weight: f32` (default 0.0); `grass_gradient` climbs
  `forage(n) + freshness_weight·freshness(n)`. `freshness_weight 0` reproduces today's
  biomass gradient exactly.
- The Survival Score's pull penalty is keyed off the dialed `cross_ratio` (via `CROSS_REF`),
  so payout falls as the pull slider rises and a zero-pull crossing is unpenalised;
  `score.pull_share` still updates for the HUD.
- `GreenWave`, senescence, `wave_grow`, `forage()`, the shrub field, and the migration pull
  all still exist and behave as before; `strength 0` remains the wave off-switch.
- The preset table (`PRESETS`) is unchanged and still loadable; its values are due for a
  re-sweep but its shape and API are stable.
