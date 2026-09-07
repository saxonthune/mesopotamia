---
title: Elk Decision Model
summary: The conceptual model under an elk's movement — a small per-elk state machine (graze / travel / cross) driving steering behaviours that descend a forage value field. Migration is a directed goal each elk weights by its own confidence (leaderless leadership), polarised into a column by forward-biased cohesion, never a hard-coded compass; the river is a bounded bet; energy is the common currency; direction is always a property of the forage field, so it emerges and would change if the world did.
tags: [ecs, modeling, decision, energy, boids, steering, fsm, flocking, migration, dialectics]
deps: [doc03.01.01, doc03.01.07, doc03.01.09]
---

# Elk Decision Model

Each tick an elk turns the local world into one whole-cell move. This doc explains *how* — the conceptual model the herding code (doc03.01.09's plan-then-steer architecture) realises. Where doc03.01.07 explains the ECS machinery the choice is stored on, this doc explains what the choice *is*. The model's spine is energy: every action is a transformation of energy from one form to another, and movement descends toward where that transformation pays best.

## The choice is a state, then a steer

An elk does not re-decide its whole destination every tick — that has no commitment in *time*, only in space, and at herd scale reads as a particle cloud that jitters in place. Instead it holds **one bit of committed state — its mode** — and the mode shapes a steer:

- **Graze** — area-restricted search: hold roughly in place and feed.
- **Travel** — directed relocation: move toward richer forage.
- **Cross** — committed river traversal: lock onto the far bank and pay the swim.

The mode is the whole state machine; there is no stored target (not a cell, not another animal). A target would dangle — the patch is eaten before arrival, the neighbour walks off — forcing replan and arrival-detection special cases, and would split steering into two arbitrating authorities. The mode is a single self-correcting bit re-grounded against the live world every tick. Each mode drives **steering behaviours** — Arrive, separation, cohesion, alignment — that blend into a desired heading, which is then quantised to one of the grid steps and committed as a whole-cell move. Committing the entire step, rather than re-rolling a heading every tick, is the hysteresis that removes the buzzing; the renderer slides `prev_cell → cell` so the discrete move looks smooth.

## Steering is descent of a value field

The desired-heading vector is not fundamental. It is the gradient of an implicit **value field** `V` over position — the worth of occupying each neighbour cell next tick. A force is the slope of a potential; movement is descent of that potential. The vector is the convenience that lets an elk sample a *local slope* rather than enumerate every object on the grid — which is why the model never collates every target in the world: to choose among neighbours it needs only the local derivative.

The feeding decision is a slope of the *same* potential along a **different axis** — not `∂V/∂position` but `∂V/∂store`, the value of converting external grass into the internal energy store. Spatial drives and feeding feel like different kinds of thing only because they are partial derivatives of one potential along orthogonal axes (space vs. internal store). Feeding is decoupled from the mode: an elk feeds whenever food sits underfoot and it is not mid-crossing, so movement state can never starve a herd standing on food.

## The three flocking rules

The social part of `V` is Reynolds' flocking, all three rules, in the book's weight order **separation > cohesion > alignment**:

- **Separation** — inverse-square repulsion from close neighbours: a minimum spacing, so the herd never stacks. Coincident elk (zero distance, where inverse-square skips them) get a deterministic golden-angle escape kick so a stack always splits.
- **Cohesion** — a pull toward the local herd centroid, with a dead zone so a well-placed elk feels no tug and only a drifted one is reeled back. The pull is **local** (a neighbourhood radius), and a fully detached straggler falls back to steering at its single nearest herd-mate, so a lost tail always has a heading home.
- **Alignment** — steer toward the mean heading of moving neighbours (velocity matching). This is the rule that lets the group *translate together* as a rolling column instead of collapsing to a blob; scaled by neighbour coherence, so a scattered herd feels little and a coherent one locks in.

Separation and cohesion alone produce a blob that either freezes or churns. Alignment is the third leg, but it is not sufficient on its own — without a shared **goal direction** a flock with alignment settles into a slowly-rotating *mill* (see below), not a march.

## Directed travel and leaderless leadership

The heart of migration is one term: each elk's **own goal** — Arrive toward the richest forage it perceives — weighted by its **confidence** in that goal. Confidence is `signal / (signal + reference)`: zero with no forage signal, rising toward one for a steep local gradient. It is the blend weight that makes a well-informed elk lead and a poorly-informed one follow — leadership as an emergent weight, not a role (Couzin et al. 2005: an informed minority steers a group with no signalling, no designated leader).

For this to work the **reference must sit in the range of signals elk actually encounter**. If the reference dwarfs the typical gradient, confidence collapses to ≈0 for *every* elk at once: the goal term is annihilated, only flocking remains, and the herd falls into the **mill** — a churning, undirected flock that advances, if at all, only as the slow drift of its own asymmetry. The mill is the torus regime of collective motion (Couzin et al. 2002: swarm / torus / polarised parallel group); it looks like every animal orbiting a tight loop while the mass leaks slowly up-gradient. Migration by mill is fragile: it is not a decision, it is an accident of churn, so anything that calms the churn also removes the drift.

Scaling the reference to the real signal range does two things at once: it lets an elk *act* on what it perceives (directed travel, not muted drift), and it makes confidence **vary across the herd** — which is the precondition for the emergent leadership. An elk on a steeper local gradient weights its goal more and de-facto leads; a flat-gradient elk weights cohesion more and follows. The heterogeneity comes from the forage field itself (the green-up front is steeper at its edge), so the column's head emerges where the world is richest, never at a chosen animal.

## Polarisation: a column, not a blob

A directed goal is necessary but not sufficient — symmetric cohesion still reels a pioneer back to the rear mass before it can advance, and the group stays a churning blob. The tip from torus to **polarised march** is **forward-biased perception**: an elk weights neighbours *ahead* of its heading more than neighbours behind (anisotropic attention, Couzin et al. 2002; real ungulates attend forward). A herd then coheres front-to-back in segments rather than all-to-one-grand-centroid, and the group elongates into a column whose front is free to pursue the goal while the body follows. This is the single change that most directly turns the mill into a march.

## The goal is the gradient of the forage field

The thing an elk Arrives toward is the **forage field** — standing crop plus the green-up front (the freshness signal of recent regrowth). An elk reads it as a local gradient and a longer sightline, and across water it peeks to the far bank. Direction is never named in the steer: it is wherever the field rises. Forward motion *emerges* because a herd grazes from behind — the richest reachable cells lie ahead, so the climb runs forward and the mass rolls (the leapfrogging wave: rear animals overtake the cropped front, graze it, are overtaken in turn — a depletion feedback loop no animal plans).

Local perception is **nearsighted**: a herd far from the green-up front, or one with a river between it and richer ground, cannot see its goal, and steering on a local gradient alone cannot solve a long-range goal. The principled *global form* of this same goal is a **forage flow field** — a diffused potential over the grid, read locally as a goal direction, with water as high cost so the field routes along fordable paths. A flow field keeps direction a property of the world at any range: it is the gradient of *where the forage is*, computed once and read locally, never a coded heading. It is the natural extension of the local-gradient goal, not a different mechanism — the same descent, sighted further.

## Crossing: the far bank as a bounded bet

A river is a gap in the forage field — water carries no forage, so the gradient at the bank points *away*, back toward dry grass. Left there, a herd never crosses. The crossing is a deliberate bet against that pull, evaluated independent of mode or heading: whenever adjacent water hides far-bank forage that out-scores staying, an elk commits to the ford on the forage merits alone — even a content grazer beside a river it has reason to cross.

- **The far bank is perceived across the water.** Beyond the local radius an elk looks across the contiguous channel to the first dry cell and reads its attractiveness — standing forage plus the green-up front. The bet is driven by the **freshness of the far bank**: a fresh far bank pulls a crossing the way fresh grass ahead pulls a step. The look reaches past the widest channel, so the barrier is a decision, not an opaque wall.
- **The cost is bounded, not linear.** A crossing is one committed effort, so the price weighed against the far bank **saturates** with width: a one-cell stream is nearly free, a ford is cheap, a wide channel asymptotes to a fixed reluctance rather than infinity. Width deters but never forbids — elk swim rivers. (The per-tick swim energy drain is a separate, real cost; this bound shapes only the choice.)
- **Commitment carries it across.** Mid-channel the two banks beckon equally and a memoryless field would dither in a limit cycle. The Cross mode supplies the commitment: once begun, the elk Arrives hard at the locked far bank and ignores the herd's backward pull, so the crossing runs to dry land. On landing it does not immediately re-cross — the lasting swim cost means recrossing demands a far bank genuinely, not marginally, greener.

## Graze: hold and feed

With migration carried by directed Travel, Graze is the hold-and-feed state: stay near the herd and convert forage underfoot. It runs the same flocking blend — separation for spacing, cohesion toward the (forward-biased) local centroid, alignment to neighbours — so a grazing herd holds a loose clump rather than diffusing.

The leave→travel edge is a **per-elk marginal-value judgment**, not a shared clock: an elk relocates once feeding *here* stops paying — its own recent intake rate has fallen below a bar that **scales with its hunger** — *and* a meaningfully richer cell is in reach. Two things make that intake rate fall as a cell is worked over: feeding follows a **saturating functional response** (fresh forage feeds fast, grazed-down forage slowly), so a thinning cell is sensed directly through the gut. A fed elk sets a high bar and moves on as soon as its cell thins; a hungry one lowers its bar and lingers on poor ground. Because the trigger is each elk's *own* local intake and not a herd-wide timer, elk at the depleting front cross their bar first and move on while those behind, on already-cropped ground, linger — the herd **desynchronises**, and a rolling front emerges instead of the whole mass flipping graze↔travel in lockstep. Gating on *reachable* improvement — never an absolute target — is load-bearing the other way: an absolute "travel until you reach rich grass" bar is a one-way trap, because on thin terrain *no* cell clears it, so the elk dashes forever and the herd starves.

The intake bar holds Graze at the **mode** level — an elk keeps feeding while the cell still pays, rather than flicking straight back into Travel. The remaining ambition is rest at the **steer** level: a content grazer with space and a place near its herd settling flat, taking no step at all (priority arbitration: separation and cohesion each yielding a clean zero when neither is active, so there is no rotating residual to orbit). That clean steer-rest is sound only once graze cohesion is weak enough not to re-lock the herd in place.

## Energy is the common currency

What makes every drive comparable is one currency — **energy and its conversion between forms**:

| Action | Energy transformation |
|---|---|
| Travel / move | spend stored energy → displacement (partly dissipated); justified only if the destination's value exceeds the cost |
| Graze | convert external potential energy (grass — fixed sunlight) → internal store |
| Rest | **conserve** — basal expenditure only, no conversion |

A sated elk's `∂V/∂store → 0` — nothing to gain by converting more grass — so it rests. A starving elk's `∂V/∂store` is large, so feeding dominates the whole field, including the cost of crossing a river it would otherwise never enter. Hunger is not a force added to the others; it **steepens one axis of the potential**. This carries the quantity → quality threshold: hunger accumulates tick by tick (a quantitative drip) until the food axis is steep enough to swamp the rest, and behaviour qualitatively flips — from "graze with the herd" to "ford the river toward forage."

## The principled limit: direction is the world's, not the code's

The temptation, faced with a stalled migration, is a `migration = east · strength` term — bias every elk one way. It would pass every test and look like a clean column. It is the one thing the model must never do: it **hard-codes the phenomenon**, making the herd march east because the code says *east*, not because the world pulls it there — blind to forage, the green-up front, and rivers.

The faithfulness test is concrete: **rotate the map, or move the green-up front north — the herd must follow with no code change.** Migration must remain a property of the forage field, never a constant in the steer. By that test the coded-vector fails; the mill technically passes (no coded direction) but fails the deeper bar, because its migration is an accident of churn rather than a decision; and the directed model passes — every direction is read from the forage / flow field and weighted by each elk's confidence, and nothing in the steer names a compass.

That is the line the whole model holds to: **keep the cause in the world** (the forage and green-up fields) **and the mechanism in local per-elk rules** (perceive → weight by confidence → blend with flocking → commit the crossing). Migration then emerges, and would change if the world changed. Not every drive reduces to energy — cohesion and separation are a behavioural prior (herd safety, no predation modelled), layered phenomenologically on top. But the more each drive is expressed as an expected energy consequence, the more the steer collapses into descent of a single energy potential. Energy is the spine; herd-feeling is soft tissue not yet ossified onto it.

## Terminology

- **Mode (graze / travel / cross)** — the one bit of committed state per elk; the whole state machine. Re-grounded against the live world every tick, never storing a target.
- **Steering behaviour** — a named contribution to the desired heading (Arrive, separation, cohesion, alignment); blended, then quantised to one grid step.
- **Value / potential `V`** — the implicit scalar worth of occupying a cell next tick; the steer descends it.
- **Desire / desired heading** — the gradient `∇V` over the neighbour cells; the blended steering vector.
- **Confidence** — `signal / (signal + reference)`, an elk's trust in its own forage goal; the blend weight between leading (own goal) and following (cohesion). Varies across the herd only when the reference sits in the real signal range — the precondition for emergent leadership.
- **Leaderless leadership** — the column's head emerges where the forage field is steepest, weighted by confidence, with no designated leader (Couzin 2005).
- **Mill (torus regime)** — an undirected flock that churns in place and drifts slowly up-gradient; the failure mode when the goal term is annihilated and only flocking remains. Migration-by-mill is fragile and not a decision.
- **Polarisation** — the tip from mill to marching column, driven by forward-biased (anisotropic) cohesion: cohere with neighbours ahead more than behind.
- **Forage field** — standing crop plus the green-up front; the field whose gradient is the travel goal. Direction is wherever it rises, never named.
- **Flow field** — a diffused global forage potential read locally as a goal direction; the principled cure for steering's nearsightedness, routing around water without a coded path.
- **Reachable improvement** — the directional half of the leave gate: relocate only toward a cell genuinely richer and *in reach*, never an absolute target. Gating on the reachable keeps the mode from trapping an elk in a perpetual dash on thin terrain.
- **Intake bar** — the satiation half of the leave gate: an elk leaves Graze when its own recent intake rate falls below a bar that scales with its hunger (fed → high bar, leaves sooner; hungry → low bar, lingers). Per-elk and local — there is no herd-wide clock — which is what desynchronises the herd and lets a rolling front form rather than a lockstep flip. Felt through a saturating functional response, so a thinning cell lowers intake directly.
- **Crossing bet** — the bounded-cost decision to ford: far-bank green-up perceived across the water, cost saturating with width, committed by the Cross mode so it runs to the far side.
- **Store / `∂V/∂store`** — the internal energy stock and the feeding axis of the potential; its marginal value declines to zero as the store fills, separating rest (conserve) from graze (convert).
- **Quantity → quality threshold** — quantitative hunger accumulation steepening the food axis until behaviour qualitatively flips.
- **Leapfrogging wave** — the emergent rolling grazing front; rear animals overtake the cropped front to fresh forage, coherent forward motion from a depletion loop with no per-agent target.
- **No-hard-coding test** — rotate the map / move the green-up front; a faithful model's herd follows with no code change, because direction lives in the world, not the steer.
