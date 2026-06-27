---
title: Movement Redesign Research
summary: Why the per-tick drive accumulator buzzes, and the plan-then-steer architecture the movement-ecology and game-AI literatures converge on as its successor — a small per-elk state machine driving steering behaviours (Arrive), with dead-band hysteresis against decision-propagation jitter and an emergent, leaderless follow-chain for the column.
tags: [herd, movement, steering, decision, fsm, boids, flocking, buzzing, research]
deps: [doc03.01.05, doc03.01.08]
---

# Movement Redesign Research

The elk decision core (doc03.01.08) re-decides an elk's entire destination every tick: a Boltzmann softmax over six candidates (four cardinal steps, stand, graze). At herd scale this reads as a particle cloud that jitters in place — the **buzzing** — rather than a mass that rolls gently across the map. This session diagnoses the buzzing as structural to the per-tick accumulator, and identifies the architecture two independent literatures converge on as its successor: commit to a destination, steer there, re-decide on arrival.

## The buzzing is structural, not a tuning fault

The accumulator's unit of commitment is one tick, and four mechanisms compound that into oscillation:

- **No commitment in time.** The model stores no destination and re-rolls direction every tick. The foraging-mode bit (doc03.01.08) sharpens the softmax temperature but does not commit to a *where*, so a travelling elk still re-picks its heading each tick.
- **Stochastic ties.** When two drives roughly cancel — grass-gradient against cohesion — no candidate dominates, and thermal noise picks east then west.
- **Opposed drives overshoot.** Separation and cohesion fight; an elk steps toward the centroid, overshoots, and separation pushes it back. Classic boids jitter on a discrete lattice.
- **Cardinal quantisation manufactures it.** A desire pointing north-east can only be expressed as alternating N and E steps, and noise turns that into an N/E/W stutter.

Game-AI practice names this directly. In a herding game the same effect "is caused by **propagation of decisions through a group, often amplified at each step**" — one animal moves too close to a neighbour, who moves, and so on. The prescribed cure is **hysteresis**: an animal "will only move toward [others] if they move a long way away," leaving "a range of distances in which [it] will not react at all." A dead-band, not a sharper force.

## Two literatures, one answer

**Movement ecology** models an animal as switching between discrete states — *area-restricted (intensive) search* and *directed (extensive) relocation* — not re-evaluating direction every step. The animal commits to a heading for a run, then turns; the formalism is the hidden-Markov / composite-random-walk movement model. Real elk corroborate it: herds travel as a led, single-file column between known feeding and bedding areas, leapfrog-graze the front, and make directed seasonal relocations across open ground and rivers — distinct behavioural states with clean transitions, not a continuous blend.

**Game AI** reaches the same shape from the engineering side. A herding creature is "a **simple decision-making framework controlling a portfolio of steering behaviours**" — a small state machine (graze / flock / separate / flee), each state running one steering behaviour or a simple sum of them. Because the world is open outdoor terrain, "the steering behaviours can act locally and be combined **without complex arbitration**." And the warning against over-engineering is explicit: "once a creature is able to navigate autonomously around the game world, it is typically too smart to be easily manipulated by the player" — so no pathfinding graph, no goal-oriented planner.

Both converge on the same loop, which is the redesign's spine: **pick a destination, steer to it, re-decide on arrival.**

## The architecture the research points to

An elk runs a **small state machine** — *graze*, *travel*, *cross* — and each state drives **steering behaviours** rather than a per-tick softmax:

- **Graze** — hold and feed, a slow wander pausing to eat. Transition to *travel* on a per-elk marginal-value judgment: the elk's own intake here has fallen below a hunger-scaled bar *and* a richer cell is reachable (doc03.01.08).
- **Travel** — commit to a destination (a richer patch, or a neighbour ahead) and **Arrive** at it. Arrive is the anti-overshoot primitive: two radii, an outer one where the elk slows and an inner one where it stops, so it settles onto a target instead of orbiting and oscillating the way a plain seek does. The committed destination is held across ticks and only re-chosen on arrival or when it becomes invalid — which is what structurally removes the buzzing.
- **Cross** — the existing river logic (doc03.01.08): perceive the far bank across the span, weigh `cross_desire` against the bounded `swim_cost`, and Arrive at the far bank once committed.

**Hysteresis** sits on every transition and on the separation/cohesion blend — the dead-band the literature prescribes — so a decision does not propagate and amplify through the herd. The existing `travel_margin` is one instance of this; the redesign generalises it.

### Leadership is emergent, and direction stays a game lever

The column needs no designated leader and no hard-coded compass. Two leaderless mechanisms, both standard, combine:

- **Emergent follow-chain.** Each elk runs its own Arrive toward a neighbour ahead of it; the formation (a V, a column) emerges from that local rule "in exactly the same way as flocking behaviours emerge," with no overall geometry and no leader. The result is organic "controlled disorder" — which for a natural herd is the desired look, not a defect.
- **Confidence-weighted goal-vs-cohesion** (Couzin et al. 2005). A group navigates accurately when only *some* individuals are informed, with no signalling and no role: informed individuals move toward their goal, uninformed ones stay with the group. Translated to one identical rule per elk: blend (own goal direction) with (cohesion toward neighbours), weighted by **own local-signal strength**. An elk on a steep gradient trusts itself and leads; an elk on flat ground trusts the herd and follows.

The **forage field's heterogeneity** (the freshness front of recent regrowth, doc03.01.05) breaks the symmetry: elk at the green-up front have a steep gradient and become de-facto leaders; the interior follows. "In front" never has to be computed — the confident elk move and the column forms behind them. Directionality therefore emerges from the forage field, never from a hard-coded eastward pull, which keeps migration direction a tunable part of the game (doc02.03) rather than a constant.

A heavier alternative — a *two-level formation* steered by an invisible anchor point (the herd's centre of mass nudged forward by its mean velocity, with the formation held back so stragglers catch up) — is also leaderless and directionless, but carries slot bookkeeping and a military crispness that the organic emergent follow-chain does not need here.

## What survives, and where movement lives

The redesign rewrites the decision core, not the model's measured kernels. The pure, tested value and cost functions of doc03.01.08 — `grass_gradient`, `forage_sightline`, `cross_desire`, `swim_cost`, `forage_across` — **survive**, demoted from per-tick step-scorers to *destination scorers and crossing costs*. `cross_desire` becomes "is the far bank worth pathing to"; the gradients become "which patch to Arrive at." The balance metrics and verification discipline (doc04.01) carry over unchanged.

**Position stays on the lattice; the steering ideas are ported to it.** Each state still blends Arrive, separation, and cohesion into a desired heading, but that continuous heading is *quantised to one of the eight grid steps* (`quantize_step`) and committed as a whole-cell move — so an elk always sits on a real cell, never a sub-cell drift. The continuous-velocity ideas survive as lattice equivalents: speed-near-target becomes a lower **slide rate** (`move_rate`, the fraction of a cell animated per tick, so a step spans several ticks), and commitment-in-time becomes **step commitment** — an elk cannot re-decide mid-step, which is the coarse hysteresis that, with the dead-band on the heading magnitude, replaces the continuous momentum term. The renderer turns the discrete move into smooth motion by interpolating `prev_cell → cell` by the slide progress, sub-sampled with the fixed-step overstep. This keeps grass interaction trivially cell-indexed and removes the cardinal-quantisation buzzing not by going continuous but by *committing* each quantised step instead of re-rolling it every tick.

## References

The technique inventory and the ecology grounding, for deeper work:

- Ian Millington, *Artificial Intelligence for Games* (2nd ed.) — the technique catalogue. §3.3.4 Arrive; §3.4 Combining Steering Behaviours; §3.7.3 Emergent Formations and §3.7.4 Two-Level Formation Steering ("Removing the Leader"); §13.2 Flocking and Herding Games (esp. §13.2.1 the FSM-over-steering creature, §13.2.3 Steering Behaviour Stability — the hysteresis cure).
- Reynolds 1999, "Steering Behaviors for Autonomous Characters" — the steering primitive catalogue (seek, arrive, separation, cohesion, flow-following).
- Couzin, Krause, Franks & Levin 2005, "Effective leadership and decision-making in animal groups on the move," *Nature* 433:513 — leaderless, signal-free emergent leadership.
- Nathan et al. 2008, "A movement ecology paradigm for unifying organismal movement research," *PNAS* — the state-switching movement framing.
- Grimm & Railsback, *Individual-based Modeling and Ecology* — building the simulation of it; source of the ODD protocol the verification practice (doc02.01) cites.
