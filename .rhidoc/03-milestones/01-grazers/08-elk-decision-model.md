---
title: Elk Decision Model
summary: The conceptual model under an elk's per-tick choice — a flat palette of candidate next-states each scored by one energy-grounded value function, picked by a Boltzmann softmax. Unifies spatial movement drives and the eat/rest decision as gradients of a single potential, and names the terminology the code uses.
tags: [ecs, modeling, decision, energy, boids, softmax, dialectics]
deps: [doc03.01.01, doc03.01.07]
---

# Elk Decision Model

Each tick an elk commits to exactly one action from a small palette. This doc explains *how the local world becomes that choice* — the conceptual model the decision code realizes. Where doc03.01.07 explains the ECS machinery the choice is stored on, this doc explains what the choice *is*. The model's spine is energy: every action is a transformation of energy from one form to another, and the decision selects the transformation of highest expected value.

## The palette

An elk chooses among **six candidate next-states**:

- **four moves** — step to a cardinal neighbour (N, S, E, W)
- **stand** — hold position, store unchanged
- **graze** — hold position and convert grass into the internal energy store

Movement and feeding are not separate decisions resolved in sequence. They are peers in one flat palette, resolved by one selection. A two-level hierarchy (decide *move-vs-eat*, then *move-where*) is incorrect: the worth of moving is inseparable from the direction — a step east into water and a step west into forage have opposite value — so the top level cannot be evaluated without already doing the bottom level's work. The hierarchy collapses into the flat palette.

## Everything is a scalar: the value field

The force vector is not fundamental. It is a derived, compressed representation of something more basic, and that more basic thing is the same kind of quantity the eat/don't-eat decision is.

Underneath, each of the six candidates carries **one scalar**: the value of being in the resulting state next tick. Call it `V(state)` — a **value**, or **potential**. The decision scores all six and picks the largest, softened by noise.

The spatial **desire vector** is then exactly the gradient of this value field over position, `∇V`. A force is the slope of a potential; movement is descent of that potential. The vector is a convenience for the four spatial neighbours — it lets an elk sample a *local slope* of the field rather than enumerate every object on the grid. This is why the model never collates every target in the world: to choose among local neighbours it needs only the local derivative, never the global list.

The eat decision is a gradient of the *same* potential along a **different axis** — not `∂V/∂position` but `∂V/∂store`, the value of moving the elk's internal energy stock. Spatial drives and the feeding decision feel like different kinds of thing only because they are partial derivatives of one potential along orthogonal axes of the state space (space vs. internal store). They are two slopes of one surface.

## Energy is the common currency

What makes `V` a single comparable number across all six candidates is one currency: **energy, and its conversion between forms.** The palette read thermodynamically:

| Action | Energy transformation |
|---|---|
| Move | spend stored energy → displacement (partly dissipated as heat); justified only if the destination's potential exceeds the cost |
| Graze | convert external potential energy (grass — fixed sunlight) → internal store |
| Stand | **conserve** — basal expenditure only, no conversion |

With energy as the currency the decision is legible without appeal to "desire": the elk selects the energy transformation of highest expected net value, given its current store.

### Why stand and graze are distinct

Both stand and graze hold position; they differ only in whether energy is converted. The separator is the **declining marginal value of intake**: `∂V/∂store → 0` as the store fills. A sated elk gains nothing by converting more grass, so it conserves (stands) rather than transforms (grazes). A starving elk's `∂V/∂store` is large, so the graze candidate dominates the whole palette, including every spatial move. Hunger is not a force added to the others — it **steepens one axis of the potential.**

## Selection is statistical mechanics

The pick is a **Boltzmann softmax**: `P(action) ∝ exp(score / temperature)`. `temperature` is temperature in the literal sense — thermal noise. High temperature lets noise dominate and the elk drifts diffusely regardless of the landscape; low temperature settles it into the energy minimum, greedy and ordered.

This carries the **quantity → quality threshold**. Hunger accumulates tick by tick — a purely quantitative drip — steepening the food axis of the potential. The behavioural distribution stays qualitatively the same ("graze-drift with the herd") until the landscape is steep enough that the food candidate's Boltzmann weight swamps the rest, and behaviour qualitatively flips to "bolt toward the nearest forage, across a river it would otherwise never enter." Quantitative accumulation, then a phase change in form.

### The resting case

Stand is selected when two conditions hold at once: the elk sits at a **local maximum of the spatial potential** (all four `∇V` neighbours are lower — a comfortable spot relative to the herd and terrain), *and* its hunger-grass term `∂V/∂store · intake` is below the cost of doing anything but conserve. Both simultaneously — a good position *and* no intake worth the conversion.

## The principled limit

Not every drive reduces to energy. Grass-gradient does (it is anticipated future energy — the scent of meals to convert). Water-cost does (locomotion energy plus swim drain). **Migration** is a seasonal compulsion and **cohesion / separation** are a behavioural prior (herd safety, no predation energy modelled), so these are phenomenological drives layered on top, not derived from the energy budget. Behaviour is over-determined — several determinations at once. The model's principled limit: the more each force is expressed as an expected energy consequence, the more the six-way palette collapses into descent of a single energy potential sampled thermally. Energy is the spine; migration and herd-feeling are soft tissue not yet ossified onto it.

## Code shape

The model dictates the structure that keeps the code legible:

1. **One scalar value function in one currency** (energy-equivalent) scores a candidate next-state. There is no force system and separate eat system emitting incommensurable outputs.
2. **All six candidates score through it.** Move-candidates compute their scalar via the spatial-gradient term — the desire vector, demoted to *an implementation detail of the spatial part of the score*. Stand and graze compute theirs via the `∂V/∂store` term. Same units out.
3. **One Boltzmann pick over the six scalars.**

The desire vector survives unrefactored; it stops standing in for the whole decision and becomes the cheap way to evaluate the spatial term of `V` over the four neighbours. A common currency is what lets stand, graze, and four directions share one softmax without a type mismatch, and what makes the decision a single pure, testable function rather than two systems arguing in different units.

## Terminology

- **Palette** — the fixed set of candidate actions an elk chooses among in a tick (four moves, stand, graze).
- **Candidate / next-state** — one reachable state the elk could occupy next tick; the unit a score attaches to.
- **Value / potential `V`** — the scalar worth of occupying a state next tick; the quantity every candidate is reduced to.
- **Desire vector** — the spatial gradient `∇V` over position; the compressed summary of the four move-candidates' scores.
- **Drive / force** — a single named contribution to the spatial gradient (separation, cohesion, grass, social, migration).
- **Store** — the elk's internal energy stock in `[0, 1]`; drains each tick, replenished by grazing.
- **`∂V/∂store`** — the feeding axis of the potential; the marginal value of intake, which declines toward zero as the store fills.
- **Marginal value of intake** — how much one unit of converted grass is worth at the current store; the separator between stand (conserve) and graze (convert).
- **Boltzmann softmax** — the selection rule `P ∝ exp(score / temperature)`; samples a discrete action from continuous scores.
- **Temperature** — the noise level of selection; high diffuses behaviour, low makes it greedy.
- **Quantity → quality threshold** — the point at which quantitative hunger accumulation flips behaviour into a qualitatively different mode.
- **Over-determination** — behaviour produced by several determinations at once (energy plus phenomenological drives), not a single cause.
