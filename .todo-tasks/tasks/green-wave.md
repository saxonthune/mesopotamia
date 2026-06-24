# Green Wave — a travelling forage crest that pulls the herd east

## Motivation

Today only the migration **pull** (`Drives::migration`, a flat `Vec2::X` force) can
produce sustained eastward progress. The natural drives — grass, social, cohesion,
separation — cannot, so the Survival Score's optimum is degenerate: crank the pull and
the scarcity to max and the behavioural weights barely matter. To make weight-tuning the
route to a high score, the *natural grass drive itself* must be able to carry the herd
across the map.

The green wave does this. Real migratory herbivores surf a wave of spring green-up: a band
of fresh, high-quality forage that sweeps across the landscape, and the herd that tracks the
freshest grass moves with it. We model that as a **travelling biomass crest** in the grass
field: grass greens up in bands that march east, and ungrazed grass behind a crest
**senesces** (slowly decays), so the standing-grass profile is a moving bump. The existing
`grass_gradient` drive (movement.rs) then points east with *no new drive* — following peak
grass *is* migrating. This is "Option A" from the triage discussion, chosen over the
regrowth-timing-only "Option B" because B's signal (a tiny per-tick regrowth delta) is
swamped by any static shrub clump, whereas A's crest swings standing grass across the full
capacity range and so competes with shrub biomass on equal footing.

## Do NOT

- **Do NOT touch the shrub field, `grow_shrubs`, or `SHRUB_REGROW`.** The wave modulates
  *grass* regrowth only. Shrubs stay as authored lateral texture.
- **Do NOT change `forage()` (grid.rs:314) or `grass_gradient` (movement.rs).** The wave
  works *through* the existing grass-seeking drive, not by adding a new drive or a new
  steering term. If you find yourself adding a migration-like vector, stop — that's the
  wrong approach.
- **Do NOT add senescence to the shrub field** — shrubs regrow in place; only grass senesces.
- **Do NOT break the zero-amplitude identity.** At wave strength 0 the system MUST reduce
  *exactly* to today's behaviour: floor == `rate.intrinsic` everywhere, no senescence. This
  is what keeps the change safe and is a required test.
- **Do NOT key the score's difficulty off the new params.** Difficulty still keys off
  `regrow_ratio` (score.rs). The wave is orthogonal. Leave score.rs alone — that's a later
  chain phase.
- **Do NOT run `cargo test` / `just test-all` in the loop** — it links Bevy and has frozen
  the machine. Use `just test-fast`. Run `test-all` once at the very end.
- **Do NOT widen `spread_grow`'s existing scalar-floor signature in a way that forces edits
  to `grow_shrubs`.** Add a new per-cell variant and keep `spread_grow` as a thin wrapper so
  the shrub caller and its tests are untouched.

## Plan

### 1. Pure wave field in `field.rs`

`green_wave` and senescence are lattice-field algorithms, so they belong in `field.rs`
beside `spread_grow`/`smooth_axis`.

Add a pure function for the crest field:

```rust
/// Travelling green-up crest at a column, in [0, 1]. Multiple bands march east as
/// `tick` rises: phase = 2π·(col/wavelength − tick·speed). `speed` is cycles per
/// tick (crest advances `wavelength·speed` cells/tick). At `amp == 0` callers use
/// `base` directly and never call this — but the field itself is well-defined for all
/// inputs. Returns (cos(phase)+1)/2 so the crest is 1 and the trough 0.
pub fn green_wave(col: usize, wavelength: f32, tick: f32, speed: f32) -> f32 { ... }
```

Add a per-cell-floor growth variant so the wave can drive regrowth without disturbing the
scalar `spread_grow` (which `grow_shrubs` still calls):

```rust
/// Like `spread_grow` but with a per-cell intrinsic `floor` and a per-cell `senesce`
/// rate. Each cell grows toward `cap` at `floor[i] + spread·cover`, then relaxes toward
/// 0 at `senesce[i]` (applied to the post-growth value). `floor.len() == senesce.len()
/// == field.len()`. Double-buffered, order-independent, same as `spread_grow`.
pub fn wave_grow(field, cap, width, height, floor: &[f32], spread, senesce: &[f32]) -> Vec<f32> { ... }
```

Refactor `spread_grow` to delegate (build a constant `floor`/`senesce` vec, or keep
`spread_grow`'s body and have `wave_grow` generalise it — whichever keeps `spread_grow`'s
existing tests green without edits). Senescence must be a *gentle* relaxation toward 0
(`next = grown·(1 − senesce[i])`), not a hard cut.

Tests in `field.rs` (verification skill — concrete + metamorphic, name the breaking input):
- **crest advances east**: `argmax_col(green_wave over a row)` strictly increases between
  `tick` and `tick + dt` for `speed > 0`.
- **periodic in col**: `green_wave(col, λ, t, s) ≈ green_wave(col + λ, λ, t, s)` (within fp).
- **bounded**: `green_wave ∈ [0, 1]` over a sweep of inputs.
- **identity**: `wave_grow` with `floor = [k; n]`, `senesce = [0; n]` equals
  `spread_grow(field, cap, w, h, k, spread)` cell-for-cell.
- **senescence decays troughs not crests**: with a high-`senesce` cell and a zero-`senesce`
  cell at equal starting grass, after one step the high-senesce cell holds less grass.
- **water cells stay empty**: `wave_grow` zeroes `cap <= 0` cells just like `spread_grow`.

### 2. `GreenWave` resource (the two sliders) in `grid.rs`

Add a small resource beside `GrowthRate`:

```rust
/// Tunables for the travelling green-up crest. `strength` 0 disables the wave
/// entirely (floor == intrinsic everywhere, no senescence — today's behaviour).
#[derive(Resource)]
pub struct GreenWave {
    pub strength: f32,   // 0 = off; scales crest depth AND senescence together
    pub speed: f32,      // cycles/tick the crest marches east
    pub wavelength: f32, // crest spacing in cells
}
```

Sensible defaults: `strength` small but visible (start ~0.5), `speed` slow (a crest crosses
the map over a few hundred ticks — derive from `GRID_WIDTH`), `wavelength` ~⅓–½ of
`GRID_WIDTH` so a couple of bands span the map. Tune these in the harness (step 4) before
finalising; pick values where the wave-on herd out-travels wave-off at **zero pull**.

`init_resource::<GreenWave>()` in `GridPlugin::build`.

### 3. Wire the wave into the `growth` system (`grid.rs`)

`growth` needs the sim tick as a phase clock. `Spawner.elapsed` (elk module) already counts
elapsed ticks and is incremented each wave-spawn — thread it in, or if that coupling is
awkward, add a tiny `FixedUpdate` tick counter resource in `grid.rs`. Prefer the existing
clock if clean.

Rewrite `growth` to build per-cell `floor` and `senesce` from `GreenWave` + the wave field,
then call `wave_grow`:

- For each cell at column `col`: `w = green_wave(col, wavelength, tick, speed)`.
- `floor[i] = rate.intrinsic · (1 + strength·(crest_gain·w − trough_cut·(1−w)))` — i.e. the
  crest regrows faster than `intrinsic`, the trough slower. Clamp `floor[i] ≥ 0`. Choose
  `crest_gain`/`trough_cut` so the *mean* floor over a wavelength stays ≈ `intrinsic`
  (the wave redistributes regrowth in space/time, it doesn't change the global budget — this
  keeps `regrow_ratio`/difficulty semantics intact).
- `senesce[i] = strength · senesce_max · (1 − w)` — decay strongest in the trough (behind a
  crest), ~0 at the crest.
- **`strength == 0` ⇒ `floor[i] == intrinsic`, `senesce[i] == 0`** for every cell. Assert
  this path is exact (the identity test in step 1 covers `wave_grow`; a small system-level
  reasoning note in the code comment suffices here).

Keep `grow_shrubs` exactly as-is.

### 4. Harness diagnostic — prove the natural drive crosses (the whole point)

In `tests/herd_shape.rs`, add an **`#[ignore]` diagnostic** (not a hard gate — emergent,
seed-noisy) that measures the wave's purpose: with the **migration pull at zero**
(`cross_ratio`/`migration` → 0, or `params.migration = 0`), the herd's net eastward
centroid displacement is **greater with the wave on than with it off**.

- Run it on the **real worldgen map** (`sim_harness::diagnose_worldgen`) so the existing
  shrub field is present — this is the shrub-pinning guard. If `diagnose_worldgen` can't yet
  toggle `GreenWave`, extend the harness minimally to set `GreenWave.strength` (and zero the
  pull) for the run. Report both centroid-col trajectories.
- Also add a quick bare-`open_plain` variant for a clean read without shrubs.
- This diagnostic is for tuning step 2's defaults and for the eventual write-up; it prints
  numbers. Keep it `#[ignore]`, reproducible (seed it), and assert only a loose envelope (or
  just print) — do not pin a brittle exact value.

If, after tuning, the wave-on herd does **not** out-travel wave-off at zero pull on the
worldgen map, that's the signal the amplitude is too low or shrubs are pinning — raise
`strength`/lower shrub influence in the harness and note the finding in the doc (step 5).

### 5. Document

Update the worldgen/forage doc that owns regrowth (find it via `.rhidoc/MANIFEST.md` — the
grass reaction-diffusion / vegetation doc). Add a present-tense section describing the green
wave: standing grass forms east-marching crests; ungrazed grass senesces behind them;
following peak grass is migrating; `strength 0` is the off switch; the wave redistributes
regrowth without changing the global budget (so difficulty is unaffected). No temporal/MVP
prose. If the right doc is genuinely absent, add one line via the `rhidoc` CLI — do not
invent a sprawling doc.

## Files to Modify

- `src/field.rs` — `green_wave`, `wave_grow`, refactor `spread_grow` to delegate; tests.
- `src/grid.rs` — `GreenWave` resource + default + `init_resource`; rewrite `growth` to
  build per-cell floor/senesce and call `wave_grow`; thread the tick clock.
- `src/elk/components.rs` *or* a new tick resource — only if `Spawner.elapsed` can't be read
  cleanly from `growth`; prefer reusing the existing clock.
- `tests/herd_shape.rs` — `#[ignore]` green-wave-crosses diagnostic (worldgen + plain).
- `src/sim_harness.rs` — only if needed to toggle `GreenWave`/zero the pull in a diagnose run.
- `.rhidoc/...` — the regrowth/vegetation doc gets a green-wave section.

## Verification

```bash
just test-fast
```

All lib unit tests pass, including the new `field.rs` wave tests and the preserved
`spread_grow`/`grow_shrubs` tests. Then once, before hand-off:

```bash
just test-all
```

Run the ignored diagnostic manually to tune defaults and confirm the herd crosses on the
natural drive (not required to be green — it prints):

```bash
cargo test --test herd_shape green_wave -- --ignored --nocapture
```

## Out of Scope

- The **pull-penalty** score change (next chain phase) — score.rs is untouched here.
- **HUD** surfacing of the wave phase / sliders polish (later chain phase). Adding the two
  raw sliders to the existing controls panel is fine if trivial, but no new stats widgets.
- Senescence as a separate independent slider — `strength` scales crest and senescence
  together for now; splitting them is a future deepening only if tuning demands it.

## Notes

- The key risk is **global forage budget drift**: if the crest adds more regrowth than the
  trough removes, the wave secretly makes the world easier/harder and muddies the
  `regrow_ratio` difficulty. Keep the wavelength-mean floor ≈ `intrinsic` (step 3) and
  sanity-check total standing grass (`grid.total_grass()`) is stable wave-on vs wave-off at
  the same `regrow_ratio` in the harness.
- Senescence vs the camper: senescence is the pressure that stops a herd from parking on
  stale grass — intended. But too strong and it mass-starves. Tune `senesce_max` so a
  *grazing* herd on a crest stays fed; only *ungrazed* trough grass decays meaningfully.
- Reproducibility, not bit-determinism: seed the diagnostic so a failure replays; assert an
  envelope, not an exact value (verification skill).

## Surface after this phase

- `field::green_wave(col, wavelength, tick, speed) -> f32` — crest field in [0,1], pure.
- `field::wave_grow(field, cap, width, height, floor: &[f32], spread, senesce: &[f32]) -> Vec<f32>`
  — per-cell-floor + per-cell-senescence growth step; `spread_grow` still exists and is now
  a thin wrapper with unchanged behaviour and signature.
- `grid::GreenWave { strength, speed, wavelength }` resource, `init_resource`d in
  `GridPlugin`; `strength == 0` reproduces pre-wave behaviour exactly.
- The `growth` system regrows grass through the wave; `grow_shrubs` and the shrub field are
  unchanged and can still be relied on.
- `forage()` and `grass_gradient` are unchanged: the herd still steers up `grass + shrubs`;
  the wave acts purely by reshaping the grass term over space and time.
- Score / difficulty (score.rs) untouched — the pull-penalty phase can still assume the
  score keys difficulty off `regrow_ratio` only.
