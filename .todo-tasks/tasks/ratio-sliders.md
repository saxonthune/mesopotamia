# Convert magnitude sliders to ratio sliders

## Motivation

`ElkParams`/`GrowthRate` sliders tune absolute magnitudes, but per Parameter Discipline (doc04.02) behaviour is governed by *dimensionless ratios of competing rates*. Expose the governing ratio as the slider and derive the absolute parameter, so tuning acts on the quantity that actually sets the regime and defaults stay off the bifurcation edge.

Scope to the three tractable groups (perception÷feature-size needs a worldgen input and is left as magnitude sliders).

## Do NOT

- Do NOT change movement behaviour or the patch-leaving gate from the previous phase. This phase only changes *how three existing parameters are set from the UI* — at default ratios the derived parameters must equal today's defaults exactly, so behaviour is unchanged.
- Do NOT convert perception radii or the patch-leaving params to ratios.
- Do NOT relabel/expand other UI text — the labels pass owns that. (Author the new ratio sliders with clear labels, but don't sweep the rest.)
- Keep the verification gate at `just test-fast` only.

## Plan

The three governing ratios (doc04.02), each deriving one absolute parameter from anchor params that stay as plain sliders:

- **intake-per-bite ÷ drain** → derives `graze_yield` from anchors `bite`, `energy_drain`.
  `bite_ratio = bite·graze_yield / energy_drain`  ⇒  `graze_yield = bite_ratio·energy_drain / bite`.
- **regrowth ÷ drain** → derives `GrowthRate.intrinsic` from anchors `energy_drain`, `graze_yield`, reference capacity `MAX_GRASS`.
  `regrow_ratio = intrinsic·MAX_GRASS·graze_yield / energy_drain`  ⇒  `intrinsic = regrow_ratio·energy_drain / (MAX_GRASS·graze_yield)`.
- **migration ÷ crossing cost** → derives `ElkParams.migration` from anchor `water_cost`.
  `cross_ratio = migration / water_cost`  ⇒  `migration = cross_ratio·water_cost`.

### 1. Ratio controls + pure derivation

Add `src/elk/ratios.rs`:
- `RatioControls { pub bite_ratio: f32, pub regrow_ratio: f32, pub cross_ratio: f32 }` (Resource), registered in `ElkSimPlugin`.
- Pure functions (with the inverses above): `graze_yield_from(bite_ratio, bite, energy_drain)`, `intrinsic_from(regrow_ratio, energy_drain, graze_yield, cap_ref)`, `migration_from(cross_ratio, water_cost)`.
- `RatioControls::default()` must be the *inverse* of the current default magnitudes so derivation reproduces them: with `bite=0.5, energy_drain=0.004, graze_yield=0.035, MAX_GRASS=<the const>, water_cost=2.0, migration=0.7`, the default ratios reproduce `graze_yield=0.035`, `intrinsic=0.02`, `migration=0.7`. Compute and hard-code those default ratios.

### 2. Derivation system

Add a system (FixedUpdate or Update) that each tick reads `RatioControls` + the anchor params and writes the three derived values into `ElkParams.graze_yield`, `ElkParams.migration`, and `GrowthRate.intrinsic`. Derive in dependency order: `graze_yield` first (needed by the regrowth ratio), then `intrinsic`, then `migration`.

### 3. Rewire the sliders

In `src/ui.rs`: replace the three magnitude sliders — `energy/grass eaten` (graze_yield) and `migration (fallback)` in `behaviour_tab`, and `regrowth / tick` (growth.intrinsic) in `grass_items` — with sliders bound to the `RatioControls` fields. Keep `bite`, `energy_drain`, `water_cost`, `spread`, and fertility as plain magnitude sliders (they are anchors / have no governing ratio here). Give the new sliders clear labels.

### 4. Reconcile docs

Update Parameter Discipline (doc04.02) so the named ratios match the slider/derivation names exactly. Touch Balance Metrics (doc04.01) only if a break-even/sustainability equation needs to reference the ratio form.

### 5. Tests

Pure tests in `ratios.rs`: each derivation round-trips (ratio → param → ratio), and `RatioControls::default()` reproduces the documented default magnitudes within `1e-6`.

## Files to Modify

- `src/elk/ratios.rs` — new: `RatioControls` + pure derivations + tests.
- `src/elk/mod.rs` — `mod ratios;`, exports, register `RatioControls`, add the derivation system.
- `src/ui.rs` — rewire the three sliders to `RatioControls`.
- `.rhidoc/04-modeling/02-parameter-discipline.md` — name the ratios as implemented.

## Verification

```bash
just test-fast
```

## Out of Scope

- Perception-radius ratios (worldgen feature size).
- The labels pass (next phase).

## Notes

- `MAX_GRASS` is a const in `src/grid.rs`; use it as the reference capacity.
- At default ratios the sim must be byte-for-byte the same regime as before — verify the default-reproduction test.

## Surface after this phase

- `src/elk/ratios.rs` exists with `RatioControls { bite_ratio, regrow_ratio, cross_ratio }` (Resource) and pure `graze_yield_from`, `intrinsic_from`, `migration_from`.
- A derivation system writes `ElkParams.graze_yield`, `ElkParams.migration`, `GrowthRate.intrinsic` from `RatioControls` each tick; default ratios reproduce the prior default magnitudes.
- In `src/ui.rs`, the three former magnitude sliders (graze_yield, migration, regrowth/tick) are now ratio sliders bound to `RatioControls`; `bite`, `energy_drain`, `water_cost`, `spread`, fertility remain magnitude sliders. The patch-leaving sliders (`intake_smoothing`, `giving_up`, `leave_boost`) and perception-radius sliders are unchanged magnitude sliders.
- doc04.02 names the three implemented ratios.
- All movement/behaviour from the prior phase is unchanged at default settings.
