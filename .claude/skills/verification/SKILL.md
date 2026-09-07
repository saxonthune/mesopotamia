---
name: verification
description: Helps write verifications for emergent simulation behaviour — extract a pure metric, then assert how it must behave. Grounded in metamorphic testing and agent-based-model V&V. Use when adding or hardening a sim behaviour (movement, growth, energy, balance) and you want a test that actually pins the intent.
---

# verification

You help the user verify **emergent simulation behaviour** — the kind that has no single right answer for any one run. This is the *oracle problem*: an agent-based sim's output is emergent and seed-dependent, so you can't assert "the output is X." You can still assert how the output must *behave*.

This skill is a seed. It captures the way we verify today and the moves we already trust. Grow it as the sim grows — add a technique when a real behaviour demands one, not before.

## The one move everything rests on

**Extract the behaviour into a pure metric, then assert how that metric behaves.**

A pure metric is a plain `fn` of local state — no Bevy, no ECS, no `Grid`, no RNG. It takes the inputs that drive a decision and returns a scalar (or small struct). Once the decision lives in a pure function, it is testable in isolation, fast, and deterministic.

Established instances: `elk::cross_desire(here, ahead, across, cross_cost)`, `field::` lattice algorithms, `ui::world_viewport`. Each was pulled out of its system so its behaviour could be pinned independently of the running app.

When asked to verify something, the first question is almost always: *what is the metric, and what's the smallest pure function that computes it?* If the behaviour is still tangled in a system, extracting it is step one of the verification, not a precondition for it.

## How to assert a metric

Reach for these roughly in order of cost and locality — cheap and local first, global and expensive later. Treat the ordering as a gradient, not a checklist; most behaviours only need the first couple of rungs, and you stop as soon as the metric is honestly pinned.

- **Concrete cases.** A handful of `#[test]`s on hand-picked inputs where you *do* know the answer. The lead herd with forage ahead does not cross; the trailing herd does. This is where most behaviours start and many of them end.

- **Metamorphic relations.** Assert how the metric must *change* between two related inputs, when you can't name either absolute output. Monotonicity is the workhorse: *more forage ahead never raises cross-desire; more forage across never lowers it; a costlier ford never raises it.* Each relation is one transform on the input and the direction the output must move. These hold without an oracle, which is the whole point. Write several — a single weak relation catches little; a spread of them boxes the behaviour in.

- **Properties over generated inputs.** When a metric is subtle enough that hand-picked cases feel thin, let the inputs be generated and assert the relation over all of them. The metamorphic relation *is* the property. Reach for `proptest` here (per-value `Strategy` objects, good shrinking) rather than `quickcheck` (per-type). This is a graduation of the rung above, not a different activity — add the dependency only when a metric earns it.

- **Invariants over a live run.** Some truths are about the whole simulation, not one metric: energy doesn't appear from nothing, a population stays non-negative, a conserved quantity stays conserved. Step the sim and assert the bound holds every tick. These catch a class of bug the per-metric tests can't see.

- **Seeded soak runs.** Build the sim headless (no rendering), run it forward N ticks from a fixed seed, reduce to aggregate metrics (population, total grass, how far herds travelled), and assert they stay inside an envelope. The agents already play themselves, so this is a *soak run* — the same move grand-strategy games make when they hand every faction to the AI and let the game run for hours to surface crashes, soft-locks, and runaway feedback. It is the regression net above the smoke test (which only asks "did it boot").

  The seed buys **reproducibility**, not bit-determinism: the same seed replays the same run *on the same build*, so a tripped invariant can be re-run and watched. That is all a single-machine sim needs. The byte-exact, CRC-per-tick determinism that lockstep-multiplayer games (Factorio, RTS) require is a netcode constraint we don't carry — don't pay for it. Reproducibility is cheap and makes failures debuggable; bit-determinism is expensive and buys us nothing here.

## Principles worth keeping

- **Verify before the feature when you can.** Writing the metric and its relations *before* the system consumes it is a design tool, not just a safety net — it forces you to state what the behaviour means. `cross_desire` exists and is fully tested before fording is wired in. This is the intended rhythm.

- **One matched metric is not enough for trust.** A behaviour that looks right by one measure can be wrong by another. When something matters, pin it from more than one angle and at more than one scale — energy balance *and* herd cohesion *and* patchiness, not just whichever was easiest. (This is the lesson agent-based-model practice calls *pattern-oriented* validation.)

- **A test that can't fail teaches nothing.** Before trusting a relation, know the input change that would break it. If you can't name one, the relation is too loose to be worth writing.

## Background

The techniques here have names and literature, worth a pointer when deepening this skill:

- **Metamorphic testing** — asserting relations between multiple executions to sidestep the oracle problem. ([overview](https://en.wikipedia.org/wiki/Metamorphic_testing))
- **Property-based testing in Rust** — [`proptest`](https://github.com/proptest-rs/proptest) vs [`quickcheck`](https://github.com/BurntSushi/quickcheck).
- **Agent-based model V&V** — verification vs validation, conservation/bounds checks, and *pattern-oriented modelling* (reproduce multiple patterns at multiple scales). ([ODD protocol](https://www.jasss.org/23/2/7.html))
- **Soak / observer runs** — grand-strategy games (Paradox's `observe` mode) hand every faction to the AI and run for hours to surface late-game crashes and runaway economies; a documented late-game crash was only reproduced by letting the AI play automatically. ([Stellaris observe](https://forum.paradoxplaza.com/forum/threads/does-using-observer-mode-mess-up-the-ai-for-all-nations-or-just-the-one-youre-spectating.970257/))
- **Determinism, two kinds** — lockstep bit-determinism (Factorio CRCs the whole sim every tick to keep multiplayer clients in sync) is a netcode requirement, not ours; run-level *reproducibility* under a fixed seed is the cheap part we keep, so failing soak runs replay. ([Factorio desync](https://wiki.factorio.com/Desynchronization))
