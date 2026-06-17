---
title: Coding Patterns
summary: How code is shaped in this project — small modules, pure metrics, feature plugins, tunables as params
tags: [patterns, code, architecture, modules, testing]
deps: [doc03.01.02]
---

# Coding Patterns

How code is shaped here. These are standing rules: applied to every code change without being asked. They exist because the easy default — one growing module that does everything — is the wrong one for this simulation, which is read, tuned, and verified far more than it is written.

## One concept per module

A module owns one concept and the state and systems that serve it. A file that grows past its concept splits rather than accumulating a second. Modules use the `foo.rs` form until they hold enough to warrant a `foo/` directory with submodules along the concept's seams — the `elk/` split into `movement`, `metabolism`, `spawn`, `color` is the worked example (doc03.01.02). A new responsibility is a new module, not a new section of an existing one.

## Decisions become pure functions

Behaviour that drives a decision is extracted into a pure free function — plain inputs, a scalar or small struct out, no ECS, no `Grid`, no RNG — and pinned with tests next to it. `cross_desire`, `step_water_penalty`, `world_viewport`, and `migration_residual` are the instances. The system calls the function; the function holds the logic. This is what makes a behaviour legible, fast to test, and safe to change: the test pins the *contract*, so the body is swappable. The verification skill is the method; this is the rule that the method produces.

## Feature plugins over a thin root

A feature is a module exposing a Bevy `Plugin` whose `build` registers that feature's resources, systems, and schedule. The composition root stays small — it adds plugins and runs. Simulation plugins carry no rendering dependency so they load headless for tests (doc03.01.02).

## Tunables are params, not constants

A value the simulation balances around is a field on a params resource with a slider, not a hard-coded constant — so it is visible, adjustable at runtime, and reachable by the balancing harness. A genuinely fixed structural constant stays a `const`; a number that shapes behaviour the player or the sweep tunes is a param.
