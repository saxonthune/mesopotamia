---
title: Reading the Simulation
summary: The observability practice that makes emergent behavior legible — a ladder from a single agent's decision up to population invariants — and the craft literature behind building simulations as aesthetic experiences
tags: [observability, debugging, metrics, tooling, instrumentation, resources, craft]
deps: [doc05.01, doc04.01]
---

# Reading the Simulation

A simulation is debugged by reading it, not by guessing at its sliders. The sliders
(`ElkParams`, the grass growth handles) are the *inputs*; the history graphs — population,
mean energy, biomass (`history.rs`) — are the *far-downstream outcomes*. Everything that
explains a behavior happens in the gap between: in the per-step decision each agent makes. When
that gap is opaque, tuning a slider is editing a function by its arguments and its eventual
return value with the body blacked out.

Legibility is the property that closes the gap: **every layer between input and outcome is
inspectable at its natural grain** — one agent, the field it reads, the population's
distribution, the conservation ledger. The techniques below form a ladder from the most local
grain to the most global. They are standing practice, reusable across any agent-based model the
project grows into, not features specific to the grazers slice.

The diagnostic test: when a behavior surprises you, which rung tells you *why*? If none does,
that rung is the next tool to build — before touching a slider.

## Decision introspection — why this agent did that

When an agent picks from options by score, that score vector for one agent on demand is the
simulation's breakpoint. `herd_move` scores four candidate steps as `desire · dir −
water_penalty` and softmaxes (doc04.01). Decision introspection exposes, for a *selected* agent,
the full breakdown each tick: the candidate cells, each one's `desire · dir`, its penalty, and
the resulting softmax probabilities.

It separates bugs that aggregate views conflate. An agent that *chooses* not to act (penalty
outweighs desire) and one that *cannot perceive* a reason to act (the desire toward that option
is near zero) look identical in a population graph and demand opposite fixes. Only the per-agent
score vector tells them apart.

## Field overlays — visualize what agents read

Agents consume *fields* derived from the terrain — a grass gradient, a water-penalty surface, an
aggregate desire vector — and steer by them. Rendering those fields as a toggleable overlay on
the grid (arrows per cell for vector fields, a heatmap for scalar ones) shows the decision
landscape the agents actually inhabit, which the rendered terrain alone never reveals.

The overlay distinguishes a *cost* problem from a *perception* problem. A drive sourced from a
local gradient within a finite radius is blind to anything beyond that radius: forage on the far
side of a wide barrier registers as a flat gradient, so the agent has no pull to cross regardless
of what the crossing costs. A flat field where you expected a gradient localizes the bug to
perception range or representation — and tells you the cost slider you were about to reach for is
the wrong knob.

## Distributions, not means

A mean erases the structure that *is* the emergent phenomenon. A herd split between sated and
starving has the same `avg_energy` as a uniformly healthy one. For any per-agent quantity, the
legible view is the **live histogram** of its distribution; the mean is reserved for quantities
already confirmed unimodal. A bimodal shape names a trapped subpopulation that no average can.

## Spatial event logging — where and in what state

A counter records *how many*; it discards the *where* and the *state*, which are the diagnostic
payload. `metabolize` despawns a starving agent and increments `deaths` (doc04.01) — the count
survives, the context does not. Structured events keep the context: each mortality (and each
other salient event) is a record of tick, cell, recent energy, recent decisions, and cause.
Plotted on the map, deaths clustered tight against a barrier with forage visible beyond it
localize a bug to a coordinate in one glance. Cluster-against-a-barrier is a signature.

## Conservation laws — make the books balance

The most unambiguous class of check. An emergent system has quantities that must reconcile, and a
violated reconciliation is a bug with no "is this intended?" ambiguity:

- **Population.** `spawned = alive + deaths + departures` (doc04.01). The cohort counters already
  hold every term; the identity is an assertable invariant.
- **Energy.** Intake from grazing, minus drain and swim cost, equals the net change in total
  energy. If the ledger does not close, there is a leak or a double-count.

These are pure functions of state, so they are pinned as metamorphic assertions in the headless
harness, the same way balance metrics are (doc05.01, doc04.01, the verification skill).

## Herd-shape metrics — clump, cloud, or wave

A soak run reduced to the right aggregate scalar tells *spatial*-emergent failure apart at a
glance, where a population or energy graph cannot. For a moving herd the diagnostic trio is
**radius of gyration** (RMS spread of agents about their centroid), **centroid drift**, and **mode
occupancy** (the fraction of agents in each behavioral mode). Their joint shape names the outcome:
gyration collapsing toward zero is a *clump*; bounded gyration with a flat centroid is a *particle
cloud*; bounded gyration with an advancing centroid is a *rolling wave*. Sampled per tick over a
headless run — on a controlled dummy map (a uniform plain, a single barrier) that isolates the
decision model from terrain, or on the real map to catch a regime the dummy cannot — they make a
whole class of bug self-announcing: a herd starving in place reads instantly as mode pinned to
"travel", net energy stuck negative, and gyration shrinking, with no need to watch it render. The
pure shape metrics live in `diagnostics.rs`; the trace runner is `sim_harness::diagnose`.

## Probe scenarios — the reproducible micro-experiment

The simulation's equivalent of a unit test: a minimal hand-built world that isolates one
question. A single agent, a strip of forage, one barrier with one crossing — does it reach the
food within N ticks? Run headless and deterministic (`sim_harness`), it asserts an intent the way
a balance metric does, and it pins that intent against future change. Paired with **ablation** —
zeroing drives one at a time over the same probe — it attributes behavior to a cause: if killing
cohesion suddenly lets the agent cross, cohesion was pinning the herd against the barrier, a fact
no single-variable view surfaces.

## The principle

Instrument every layer between input and outcome, and make each inspectable at the right grain.
Of the seven rungs only the last touches a slider; the rest make existing behavior legible so the
guessing stops. A surprising behavior is first a question for the instruments, not the tuning
knobs.

## Craft resources

The project's end is aesthetic experience and light play, not a research artifact — emergent
systems that read as alive and generate story, rather than predict the world. The literature that
serves that end:

- **The Nature of Code**, Daniel Shiffman — particle systems, flocking, cellular automata, and
  physics simulation built for visual, generative ends. Free online under Creative Commons; the
  closest match to this project's "simple rules, complex behavior" aesthetic.
  <https://natureofcode.com>
- **Designing Games: A Guide to Engineering Experiences**, Tynan Sylvester (RimWorld) — designing
  *experiences* out of interacting systems, and the apophenia that makes players read story into
  emergent events.
- **RimWorld as a story generator** — Sylvester's GDC talk on building a simulation as a narrative
  engine rather than a game, driven by an AI storyteller that shapes emergent events into arcs.
  <https://www.gdcvault.com/play/1024232/>
- **Juice and legibility** — "juice" is feedback amplified beyond mechanical necessity to make
  actions feel significant; research finds medium juice beats extreme, and that legible
  action→outcome bindings are a *precondition* for it. The same legibility this doc pursues for
  debugging is what lets a player read the system. (Juicy Game Design, CHI PLAY 2019.)
- **An Introduction to Agent-Based Modeling**, Wilensky & Rand — the formal ABM foundation, paired
  with the free NetLogo environment for prototyping rules before committing them to Rust.
- **Emergent gameplay** — simple mechanics that interact to yield unscripted behavior; Dwarf
  Fortress and RimWorld are the canonical deep simulations, and their design post-mortems are a
  steady source of systems technique.
