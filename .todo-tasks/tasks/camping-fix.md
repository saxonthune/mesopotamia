# Patch-leaving state switch (the camping fix)

## Motivation

Elk camp on locally abundant grass and won't explore or cross even while starving, because hunger *amplifies* the grass-seeking drive: in `combine_drives` (src/elk/movement.rs) `appetite = 0.25 + 0.75·(1−energy)` scales `grass` and `social` up, so the hungriest elk glue hardest to the nearest grass — the inverse of area-restricted search. Per Balance Metrics (doc04.01) and Parameter Discipline (doc04.02), a forager should leave a patch when its local intake rate drops below the habitat average, switching from intensive local search to extensive directional movement.

Add that as a **soft gate**, not a new additive force (weighted-sum cancellation is already a failure mode).

## Do NOT

- Do NOT add a new steering vector or a hard binary mode flip. The switch is a continuous gate that scales existing drives.
- Do NOT remove or weaken `cross_desire` / `forage_across` — they stay; this complements them on open ground.
- Do NOT touch the UI slider *labels* or convert sliders to ratios — separate chain phases own that.
- Do NOT change `graze` energy accounting or `EnergyFlows`; only *read* the per-tick intake already computed there.
- Keep the verification gate at `just test-fast` only.

## Plan

### 1. Per-elk recent-intake state

In `src/elk/components.rs`, add a field to `Elk`:
- `pub intake_rate: f32` — an exponentially-weighted moving average of per-tick forage intake. Initialize to `0.0` everywhere an `Elk` is constructed (spawn.rs and any test constructors, e.g. metrics.rs `make_elk`).

### 2. Update the EWMA in `graze`

In `src/elk/metabolism.rs` `graze`, the per-tick intake is the energy gained this tick (`elk.energy − before`, and `0.0` when the elk does not feed). After the feed/no-feed branches, update:
```
elk.intake_rate = (1 − α)·elk.intake_rate + α·intake_this_tick
```
where `α = params.intake_smoothing`. Compute `intake_this_tick` in all three branches (shrub, grass, none).

### 3. Habitat-average intake resource

Add a resource `HabitatIntake { pub mean: f32 }` (in components.rs, registered in `ElkSimPlugin`). Each tick, set its `mean` to a smoothed average of elk `intake_rate` across all elk (EWMA with the same `α`, or a plain mean of `intake_rate` — pick the plain mean of `intake_rate` for simplicity, since `intake_rate` is itself already smoothed). Update it in `graze` (accumulate sum + count, write mean at the end) or a tiny dedicated system ordered after `graze`. Zero elk ⇒ `mean = 0`.

### 4. The soft gate (pure, tested)

Add to `src/elk/movement.rs` a pure function:
```
pub fn forage_gate(local_intake: f32, habitat_mean: f32, giving_up: f32) -> f32
```
Returns a scale in `[0, 1]`: `1.0` when `local_intake ≥ habitat_mean` (stay/intensive), decreasing smoothly toward `0.0` as `local_intake` falls below `giving_up · habitat_mean` (leave/extensive). Use a smooth ramp (e.g. clamp of `local_intake / (habitat_mean·something)` or a logistic) — document the exact form. When `habitat_mean ≤ 0`, return `1.0` (no signal ⇒ no suppression).

### 5. Apply the gate in `combine_drives`

Extend `combine_drives` to take the gate value (compute it in `herd_move` from `elk.intake_rate`, `HabitatIntake.mean`, `params.giving_up`, and pass it in). Apply:
- scale `grass` and `social` by `gate`;
- scale the migration pull *up* as the gate closes, e.g. `migration · (1 + params.leave_boost·(1 − gate))`.
So a below-average-intake elk has suppressed grass/social and an amplified directional push — it leaves. `herd_move` passes the new argument; update all `combine_drives` callers and tests.

### 6. Tunables

Add to `ElkParams` (src/elk/components.rs) with defaults chosen *inside* a regime (doc04.02), not at an edge:
- `pub intake_smoothing: f32` — EWMA α (e.g. `0.05`).
- `pub giving_up: f32` — ratio of habitat mean below which the gate closes (e.g. `0.6`).
- `pub leave_boost: f32` — how much migration amplifies when the gate is fully closed (e.g. `1.5`).
Add sliders for these in `behaviour_tab` (src/ui.rs) next to the migration sliders (plain labels are fine; the labels pass refines them later).

### 7. Document the rule

In Balance Metrics (`.rhidoc/04-modeling/01-balance-metrics.md`), add a "Patch leaving" section: the `intake_rate` EWMA, the habitat-mean comparison, the `forage_gate` shape, and how the gate scales grass/social down and migration up. Present-tense, declarative, an equation block matching the existing style.

### 8. Tests

In movement.rs tests, add metamorphic tests for `forage_gate`: equals `1.0` when `local ≥ mean`; monotonically non-increasing as `local` falls; bounded `[0,1]`; returns `1.0` when `mean ≤ 0`.

## Files to Modify

- `src/elk/components.rs` — `Elk.intake_rate`; `HabitatIntake` resource; `ElkParams` gains `intake_smoothing`, `giving_up`, `leave_boost`.
- `src/elk/metabolism.rs` — EWMA update + habitat-mean tally in `graze`.
- `src/elk/movement.rs` — `forage_gate` (pure + tests); `combine_drives` applies the gate; `herd_move` computes and passes it.
- `src/elk/mod.rs` — register `HabitatIntake`; export `forage_gate`, `HabitatIntake` as needed.
- `src/elk/spawn.rs` (+ any test constructors) — initialize `Elk.intake_rate`.
- `src/ui.rs` — sliders for the three new params in `behaviour_tab`.
- `.rhidoc/04-modeling/01-balance-metrics.md` — the patch-leaving section.

## Verification

```bash
just test-fast
```

## Out of Scope

- Ratio sliders (next phase).
- UI labels / abbreviation expansion (later phase).

## Notes

- `herd_move` runs before `graze` each tick, so it reads the previous tick's `intake_rate`/habitat mean — fine for a smoothed signal.
- After this lands, `herds_reach_the_far_edge` must still pass and camping should visibly break (watch the abundance graph's `regrowth÷drain`).

## Surface after this phase

- `Elk` has `pub intake_rate: f32` (EWMA of per-tick forage intake), initialized `0.0`.
- Resource `HabitatIntake { pub mean: f32 }`, registered in `ElkSimPlugin`, holds the running habitat-average intake.
- `ElkParams` gains `pub intake_smoothing: f32`, `pub giving_up: f32`, `pub leave_boost: f32` (with defaults). All prior `ElkParams` fields are unchanged and still exist: `separation, cohesion, grass, social, migration, quiet, cross, sep_radius, coh_radius, grass_radius, social_radius, temperature, bite, graze_yield, graze_floor, energy_drain, mig_growth, water_cost, ford_discount, swim_drain, shrub_bite, shrub_energy`.
- `pub fn forage_gate(local_intake: f32, habitat_mean: f32, giving_up: f32) -> f32` exists in `src/elk/movement.rs`, returns `[0,1]`.
- `combine_drives` takes the gate (its signature changed); callers updated.
- `behaviour_tab` in `src/ui.rs` has plain (un-refined) sliders for `intake_smoothing`, `giving_up`, `leave_boost`.
- Balance Metrics (doc04.01) documents the patch-leaving gate.
- `cross_desire`/`forage_across` and the existing slider set are otherwise unchanged.
