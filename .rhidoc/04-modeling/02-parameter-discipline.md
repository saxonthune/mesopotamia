---
title: Parameter Discipline
summary: How the model's tunables are chosen so defaults sit in a stable regime — dimensionless ratios over magnitudes, off the bifurcation edge, separated timescales, and sensitivity analysis to find the fragile knobs
tags: [parameters, nondimensionalization, sensitivity-analysis, robustness, modeling, methodology]
deps: [doc04.01]
---

# Parameter Discipline

How the model's tunables are chosen so that defaults sit in a stable regime rather than perched
on a knife-edge, and so that tuning acts on the quantities that actually govern behaviour. The
concrete groups and equations live in Balance Metrics (doc04.01); this doc is the discipline for
picking their values.

## Tune ratios, not magnitudes

A tunable's absolute value carries no meaning on its own — behaviour is governed by *ratios of
competing rates*. The model is read in dimensionless groups: a default is set by fixing a group
to a sensible order-one value and back-solving the absolute parameter. The governing groups:

- **regrowth ÷ drain** — whether a patch refills faster than a herd strips it. Above one the herd
  can camp indefinitely; the pressure to move appears only as it falls toward one. (This is the
  `regrowth_drain_ratio` the abundance readout plots.)
- **intake-per-bite ÷ drain-per-tick** — how many ticks of metabolism one full bite buys; the
  feeding regime's break-even (doc04.01).
- **migration pull ÷ crossing cost** — whether fording is ever worth more than the water penalty.
- **perception radius ÷ feature size** — whether a herd can sense the far bank of a river at all.

Tuning `graze_yield` or `water_cost` in isolation turns a magnitude; the regime is set by where
these ratios land.

## Keep defaults off the bifurcation

A parameter whose interesting value is near zero, or near a point where opposing terms cancel,
sits on a phase boundary where a small change flips the qualitative behaviour (camp ↔ migrate).
Such a default is fragile by construction. Defaults are chosen *inside* a regime — where the
behaviour is insensitive to small perturbation — not balanced at the crossover where the slope is
steepest. The migration residual's `quiet` crossover, and a migration weight set to barely
overcome the water penalty, are exactly the knobs to keep away from their edge; a discrete mode
switch is preferred over a near-zero weight when a clean regime boundary is wanted.

## Separate the timescales

Fast processes (per-tick movement and grazing) and slow processes (migration-pressure growth,
regrowth) run at clearly separated rates so they do not resonate. What matters is the ratio of
the slow rate to the fast one, not either absolute value.

## Find the fragile knobs by measurement

Which parameters the outcome actually depends on is settled by sensitivity analysis against the
model's own metrics (survival, far-edge reach, per-capita abundance), not by intuition:

- A **Morris elementary-effects screen** over a Latin-hypercube sample ranks parameters by
  influence cheaply — the first pass that separates the knobs that matter from the inert ones.
- **Sobol indices** on the few that survive the screen quantify their main and interaction effects.

A default the screen shows the output is highly sensitive to is a default that needs either a
wider stable basin or reframing as a ratio.

## Grounding

- Nondimensionalisation and dimensionless groups — the Buckingham Pi theorem.
- Global sensitivity analysis — Morris elementary effects (screening), Sobol variance
  decomposition (attribution), Latin-hypercube sampling.
