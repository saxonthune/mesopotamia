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

## Crossing: the far bank as a bounded bet

A river is a gap in the spatial potential — water carries no forage, so the grass
gradient at the bank points *away* from it, back toward dry grass. Left there, a herd
never crosses: the backward pull always beats stepping into a foodless cell. The
crossing has to be a deliberate bet against that pull, and three things shape it.

- **The far bank is perceived across the water.** Beyond the local grass radius, an
  elk entering water looks across the contiguous span for the first dry cell and reads
  its attractiveness — standing forage *plus the green-up front* (the same freshness
  signal the local gradient and the dry-land sightline climb). So the bet is driven by
  the **green wave**: a fresh far bank pulls a crossing the way fresh grass ahead pulls
  a step. The look reaches past the widest channel, so the barrier is a *decision*, not
  an opaque wall the herd simply cannot see beyond.

- **The crossing's cost is bounded, not linear.** A crossing is one committed effort,
  so the price an elk weighs against the far bank **saturates** with the river's width
  rather than growing per-cell without limit. A one-cell stream is nearly free, a
  shallow ford is cheap (fords count for a fraction of a full cell), and a wide deep
  channel asymptotes to a fixed reluctance instead of becoming infinite. This is the
  "elk swim rivers" model: width deters but never forbids. (The per-tick swim *energy*
  drain is a separate, real cost; this bound shapes only the choice.)

- **The bet is hunger-scaled.** The whole crossing term is weighted by appetite, so a
  fed herd stays on familiar grass and only a hungry one — or one facing a far bank
  clearly greener than a depleted near side — pays to cross. This is the same
  quantity→quality threshold: hunger steepens until the far bank's pull, net of the
  bounded cost, swamps the backward gradient, and the herd fords.

Together these make the river crossable on the **natural drives** — the green wave and
hunger — rather than only under the migration pull. The pull remains a crutch the score
penalizes; the green-up far bank is the honest way across.

## Commitment: the value field has a heading

A value field that depends only on position and store is **memoryless and spatially symmetric**, and that symmetry is wrong inside a river. Standing mid-channel, both banks beckon equally and reversing costs nothing, so a purely positional `V` produces a limit cycle — the elk dips a step in, the gradient pulls it back, and it dithers in place instead of crossing.

The fix is to give `V` a small dependence on the elk's recent **heading** — the direction of its last move. Continuing that heading scores a bonus; reversing it pays a penalty of equal size. This is partly energy-grounded — turning and re-accelerating dissipates locomotion energy, and abandoning a half-finished crossing wastes the swim energy already spent — and partly the phenomenological fact that a committed animal commits. The term is gentle on land (a mild bias against pointless backtracking) and strong in water, where it must dominate the symmetric pull of the two banks so a crossing, once begun, runs to the far side.

Three behaviours emerge from this one term, with no per-elk state machine — the heading is read back from the position the renderer already stores:

- **A crossing still has to be worth starting.** Heading is zero for an elk that grazed or stood, so the decision to *enter* water is governed entirely by the crossing incentive, unchanged.
- **A started crossing finishes.** Once in water with a heading set, the in-water persistence outweighs the noisy symmetric gradients, so the elk marches to dry land rather than oscillating.
- **A crossed river is not immediately re-crossed.** On landing, the heading still points away from the water, penalizing re-entry; combined with the lasting swim-drain cost, recrossing demands a far bank that is genuinely, not marginally, greener.

## Foraging mode: grazing and travelling

A purely per-tick pick over the palette has no commitment in *time*, only in space. Re-rolled every tick by the softmax, an elk takes a step, re-decides its whole destination, and takes a barely-correlated step next tick. At herd scale this reads as a **particle cloud** — a diffuse blob that jitters in place rather than a mass that moves.

Real grazing herds do something else: they roll forward as a **leapfrogging wave**. Animals at the rear sit on grass the front already cropped, so they overtake the front to reach fresh forage, graze it down, and are overtaken in turn. The wave is not planned by any animal — it emerges from a depletion feedback loop.

The model captures this with **one bit of committed state per elk — a foraging mode**, *graze* or *travel*. The toggle is gated on **reachable improvement**, not an absolute richness target:

- **Graze** in place until the patch underfoot is drawn below `leave_frac` of its capacity *and* a meaningfully richer patch — at least `travel_margin` more, in capacity-fraction — lies within perception.
- **Travel** — a directed dash, the softmax sharpened — while such a patch remains reachable, and **settle** the moment nothing better than underfoot is in reach (the elk has arrived at the local best).

The reachable-improvement gate is load-bearing: an absolute "travel until you reach 60%-of-capacity grass" threshold is a one-way trap, because on thin or barren terrain *no* cell clears the bar, so the elk dashes forever, never feeding, and the herd clumps and starves. Gating on "is somewhere better actually reachable" means an elk surrounded by nothing better simply grazes what it has. The hysteresis that commits each phase for a run comes from the *asymmetry* of the two edges — leaving demands depletion **and** a better patch, continuing demands only a better patch — so the same local state resolves differently depending on which mode the elk is already in, without any sticky band to tune.

Travel **sharpens the softmax temperature** (commit to the best direction rather than diffuse); the heading-persistence term then carries the dash in a straight line. Grazing is **never suppressed** — a travelling elk that finds worthwhile grass underfoot eats it. That is the structural guarantee the mode cannot starve the herd: the worst a misjudged mode can do is move an elk that might have stood still, never stop it from feeding where feeding is possible.

Crucially, **no per-elk target is stored** — not a cell, not another animal. A target would dangle (the grazer it aimed at walks off or dies) or go stale (the patch is eaten before arrival), forcing re-validation, replan, and arrival-detection special cases, and would split steering into two arbitrating authorities. The mode is a single self-correcting bit re-grounded against the live world every tick. The forward direction is not stored either — it **emerges**: because the herd grazes from behind, the richest reachable cells lie ahead, so the dash climbs the depletion gradient forward, and the mass rolls. This is the same discipline as every other drive here — promote a transient impulse into a short-lived bias layered on the value field, never a hierarchy of plans.

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
- **Heading / persistence** — the direction of the elk's last move; a path-dependent term in `V` that rewards continuing it and penalizes reversing, breaking the memoryless symmetry that otherwise traps an elk mid-river.
- **Foraging mode (graze / travel)** — one bit of committed state per elk, toggled on *reachable improvement*: travel while a meaningfully richer patch is in reach, settle on the local best. Travel sharpens the pick into a committed dash but never suppresses grazing, so it shapes movement without ever starving the herd; the asymmetry between the leave and continue conditions supplies the hysteresis. Turns particle-cloud jitter into a forward-rolling, leapfrogging herd.
- **Reachable improvement** — the gate on travel: an elk moves on only while perception shows a patch richer than underfoot by `travel_margin`. Gating on what is actually reachable (rather than an absolute richness target) is what keeps the mode from trapping an elk in a perpetual dash across terrain that offers nothing better.
- **Leapfrogging wave** — the emergent rolling grazing front: rear animals overtake the cropped front to reach fresh forage, producing coherent forward herd motion from a depletion feedback loop, with no per-agent target.
