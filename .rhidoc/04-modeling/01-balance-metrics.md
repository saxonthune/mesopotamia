---
title: Balance Metrics
summary: The equations that define the demo's force decomposition, the migration residual, and the survival/journey/share objective the balancing sweep optimizes
tags: [metrics, balance, movement, migration, verification, spec]
deps: [doc03.01.01, doc05.01]
---

# Balance Metrics

The formal definitions behind the balancing goal (doc03.01.01). Each is a pure metric — a
function of local state — so each is implemented as a pure function and pinned with metamorphic
tests (doc05.01, the verification skill). This doc is the source of truth the movement code, the
UI readouts, and the balancing sweep all reconcile against.

The migration axis is **+x**: the far edge a herd migrates toward.

## Drive decomposition

An elk's step is steered by a desire vector that is the sum of five weighted contributions —
four **natural drives** and one **migration pull**:

```
desire = sep + coh + grass + social + migration
```

The naturals are weights on *normalized* directions (real ratios, scale-free); the migration
pull is absolute and axis-aligned. Each natural contribution scales by its weight, with the two
food drives sharpened by hunger:

```
appetite = 0.25 + 0.75 · (1 − energy)
sep    = ŝ_sep    · w_sep
coh    = ŝ_coh    · w_coh
grass  = ŝ_grass  · w_grass  · appetite
social = ŝ_social · w_social · appetite
```

## Natural strength

How loudly the natural drives are pulling — the sum of their magnitudes:

```
natural_strength = |sep| + |coh| + |grass| + |social|
```

## Migration residual

The migration pull is a **fallback**: it fills in only as the natural drives fall quiet. It
scales by a residual factor in (0, 1] that decays as `natural_strength` rises. `quiet` is the
crossover — the `natural_strength` at which migration is at half weight:

```
migration_residual(s, quiet) = 1 / (1 + s / quiet)
migration = x̂ · w_migration · pressure · migration_residual(natural_strength, quiet)
```

Properties (the metamorphic contract): `migration_residual(0, quiet) = 1`;
`migration_residual(quiet, quiet) = 0.5`; strictly decreasing in `s`; `→ 0` as `s → ∞`. The
fade's *shape* is swappable as long as these hold.

## Migration share

How much of an elk's pull effort is the migration ("magic") force — the instantaneous force
attribution, in [0, 1]. The denominator is the **sum of magnitudes**, so drives that point
opposite ways and cancel in the vector sum still count:

```
migration_share = |migration| / (|sep| + |coh| + |grass| + |social| + |migration|)
```

It is 0 when nothing pulls. This is the quantity the UI pie chart draws and the sweep averages
over a run.

## Survival

Over a headless run, each spawned elk ends in one of three states: still alive, **starved**
(`deaths`), or **departed** off the far edge (`departures` — the journey's success). Survival is
the fraction that did not starve, summed over all cohorts:

```
spawned  = alive + deaths + departures
survival = 1 − deaths / spawned
```

## Foraging economics

Survival is the outcome of an energy ledger: each tick an elk loses `energy_drain` to
metabolism and gains intake only while grazing. Intake is **proportional to the grass it
actually eats**, and a bite leaves a *giving-up density* — a fraction `graze_floor` of the
cell's capacity stays in the ground:

```
edible      = grass − graze_floor · capacity
bite_taken  = min(bite, edible)          (no bite when edible ≤ 0)
intake/tick = bite_taken · graze_yield   (while grazing)
```

The slider values are **derived from inequalities, not tuned in the demo**. Three handles fix the
feeding regime:

- **Break-even grazing fraction.** An elk lives only if it grazes enough of the time to cover
  drain. On full bites, `f_breakeven = energy_drain / (graze_yield · bite)`. As a patch thins
  toward the floor the bite shrinks, so the duty cycle a grazed-down cell demands is strictly
  worse — which is what forces movement.
- **Single-cell sustainability.** A camped elk eats only what regrows, `≈ intrinsic · capacity`
  per tick. For a grazed-down cell to *fail* to sustain it — closing the "camp a patch and sip
  its regrowth" exploit — the yield must satisfy `intrinsic · capacity · graze_yield < energy_drain`.
- **Giving-up density.** `graze_floor` is a fraction of capacity (not an absolute level), so the
  abandon threshold scales with site quality and marginal land is given up sooner in absolute
  terms rather than zeroed out.

Hunger is the journey's engine: an elk that cannot camp must keep moving to fresh forage, and a
herd that has grazed-down its bank is the one with the energy pressure to cross the river. The
feeding ledger and the crossing decision (`cross_desire`) are the same economy — forage gained
against cost paid.

## Counterfactual journey

The demo goal is that natural drives alone carry the herd. The journey metric is a counterfactual
double-run: run the candidate params, then run again with `w_migration = 0`. The herd passes the
journey test when it still reaches the far-edge band under the second run — migration was only a
residual nudge, not the engine of travel:

```
journey_natural = max_col_reached( run with w_migration = 0 ) ≥ EDGE_BAND
```

## The balance objective

A parameter set is **balanced** when the herd survives, completes the journey on natural drives
alone, and shows little magic force along the way:

```
balanced(params) =  survival(params)        ≥ S_floor
                 ∧  journey_natural(params)            (reaches the edge with migration off)
                 ∧  mean_t migration_share(params)  ≤ σ_max
```

The balancing sweep searches `ElkParams` over the slider ranges and keeps the sets that clear
all three. Thresholds (`S_floor`, `EDGE_BAND`, `σ_max`) start loose and tighten only when a real
regression motivates it.
