# Pull Penalty — score a foraged crossing above a pull-bought one

## Motivation

With the green wave merged, the *natural* grass drive can now carry the herd east (follow
the travelling forage crest). That removes the precondition that made this change incoherent
before: there is finally a way across that does **not** lean on the migration pull. So now we
close the degenerate optimum from the other side — make leaning on the pull *cost score*.

Today `despawn_points = credit · difficulty · POINT_SCALE`: a crossing scores the same
whether the herd earned it by surfing the green wave or bought it by cranking the migration
pull to max. We add a **pull penalty**: the payout is scaled down by how much the herd has
been relying on the migration ("magic") force. A herd that crosses on the natural drives
keeps full credit; one dragged across by the pull keeps only a fraction. Combined with the
green wave, this makes weight-tuning — not pull-maxing — the route to a high score.

## Do NOT

- **Do NOT remove or weaken the migration pull mechanic itself** (`Drives::migration`,
  `combine_drives`, `migration_residual`, `cross_ratio`). The pull stays a usable tool — we
  only make *relying* on it score less. This is a `score.rs` change, not a movement change.
- **Do NOT touch movement.rs, the drives, or `forage()`.** The only behavioural file in
  scope is `src/elk/score.rs` (+ its plugin wiring in `src/elk/mod.rs` if a new resource
  field needs nothing — it doesn't; `Score` already exists).
- **Do NOT change how difficulty is computed.** Difficulty still keys off `regrow_ratio`
  (`difficulty(controls.regrow_ratio)`). The pull penalty is a *separate* multiplier.
- **Do NOT make the penalty able to flip the payout negative or exceed full credit.** It is a
  factor in `[1 − PULL_PENALTY, 1]`. A pull-bought crossing scores *less*, never negative.
- **Do NOT attach per-elk pull data to events / change the `Event` struct.** Use a smoothed
  herd-level pull-share sampled in the score system. (Per-elk attribution is a possible future
  refinement, explicitly out of scope.)
- **Do NOT run `cargo test` / `just test-all` in the loop** — `just test-fast` only; one
  `test-all` at the end.

## Plan

### 1. A `pull_share` signal on `Score`

Add a smoothed herd-level pull-reliance field to the `Score` resource:

```rust
/// Smoothed herd-mean migration reliance in [0, 1] — the fraction of drive effort
/// that is the migration ("magic") pull. The pull penalty scales payouts down by it.
pub pull_share: f32,
```

It is sampled from `DriveSamples`: each `DriveSample` already has `migration_share()`
(migration magnitude ÷ sum of all five). Take the **mean of `migration_share()` over slots
with `count > 0`** (an empty/idle herd contributes nothing); EWMA that into `pull_share` with
a slow alpha (mirror `DIFFICULTY_ALPHA = 0.03` — this is a steady gauge, not a twitchy one).
Add `PULL_ALPHA` next to the other constants.

### 2. Pure `pull_factor`

Add a pure, tested decision function beside `despawn_points`:

```rust
/// Score multiplier for how the crossing was earned. At `pull_share == 0` (all natural
/// drive) the factor is 1 — full credit. At `pull_share == 1` (all magic pull) it falls
/// to `1 − penalty`. Linear and clamped to `[1 − penalty, 1]`: relying on the pull costs
/// score, but a pull-assisted crossing is never worthless and never scores negative.
pub fn pull_factor(pull_share: f32, penalty: f32) -> f32 {
    (1.0 - penalty * pull_share.clamp(0.0, 1.0)).clamp(1.0 - penalty, 1.0)
}
```

Add `PULL_PENALTY` constant (start at `0.6` — max reliance keeps 40% of the payout; tune if
the harness shows it too punishing or too weak). Document the choice in a line comment like
the other constants.

### 3. Apply the factor in `update_score`

In `update_score`:
- Add `samples: Res<DriveSamples>` to the system params (it's a resource in `ElkSimPlugin`,
  available in the same `FixedUpdate` set).
- Each tick, compute the herd-mean `migration_share` from `samples` and EWMA it into
  `score.pull_share` (alongside the existing difficulty EWMA).
- When folding each despawn event, scale the payout:
  `let payout = despawn_points(progress, departed, score.difficulty) * pull_factor(score.pull_share, PULL_PENALTY);`
  then EWMA into `current` and update `high` exactly as today.

Update the module doc-comment at the top of `score.rs` to describe the pull penalty in
present tense (a crossing earned on the natural drive scores full; one bought with the pull
scores a fraction; the penalty is orthogonal to difficulty). No temporal/MVP prose.

### 4. Tests (verification skill — pure metrics, metamorphic, name the breaking input)

In `score.rs` `mod tests`:
- `pull_factor` **endpoints**: `pull_factor(0.0, 0.6) == 1.0`; `pull_factor(1.0, 0.6)` ≈ `0.4`.
- `pull_factor` **monotone decreasing** in `pull_share` (more reliance never raises the
  factor) — the relation whose breaking input is "factor rose with share".
- `pull_factor` **bounded** in `[1 − penalty, 1]` over a sweep, including out-of-range shares.
- **payout falls with reliance**: for fixed progress + difficulty,
  `despawn_points(p, d, diff) * pull_factor(high_share, k)` is strictly less than with
  `low_share` — the headline property: a pull-bought crossing scores less than a foraged one.
- Keep the existing score tests green (difficulty, ewma, high-water mark, etc.).

## Files to Modify

- `src/elk/score.rs` — `Score.pull_share` field; `pull_factor` pure fn; `PULL_PENALTY`,
  `PULL_ALPHA` constants; `update_score` samples `DriveSamples`, smooths `pull_share`, scales
  payouts; module doc-comment; new tests.
- `src/elk/mod.rs` — only if needed (it isn't: `Score` and `DriveSamples` are already
  resources; `update_score` just gains a `Res` param). Verify the system still compiles.

## Verification

```bash
just test-fast
```

All lib tests pass, including the new `pull_factor` tests and the preserved score tests.
Then once before hand-off:

```bash
just test-all
```

## Out of Scope

- **HUD** surfacing of `pull_share` — the next chain phase (hud-feedback) reads
  `score.pull_share` and renders it. Do not add UI here.
- **Per-elk** pull attribution (attaching the share at despawn to the specific elk). The
  smoothed herd-level share is the first cut.
- Making `PULL_PENALTY` a runtime slider — a tuned constant for now.

## Notes

- The penalty uses the *current smoothed* herd pull-share at fold time, not the despawned
  elk's lifetime share — an approximation appropriate to a herd-level rating. Note it in the
  doc-comment so the imprecision is explicit.
- Interaction to watch: at zero pull (`cross_ratio`→0) `pull_share`→0 and the factor is 1, so
  a pure green-wave crossing is unpenalised — exactly the intent. Sanity-check in the
  green-wave harness diagnostic if convenient (not required).

## Surface after this phase

- `score::pull_factor(pull_share: f32, penalty: f32) -> f32` — pure, in `[1 − penalty, 1]`,
  monotone decreasing in `pull_share`.
- `Score.pull_share: f32` — public field, smoothed herd-mean migration reliance in `[0, 1]`,
  updated every tick by `update_score`.
- Despawn payouts are scaled by `pull_factor(score.pull_share, PULL_PENALTY)`; `current`,
  `high`, and `difficulty` semantics are otherwise unchanged. Difficulty still keys off
  `regrow_ratio` only.
- `PULL_PENALTY` constant lives in `score.rs` (not a slider).
- No movement / drive / event-struct changes: `DriveSamples`, `Drives::migration_share`,
  `forage()`, and the migration pull all behave as before.
