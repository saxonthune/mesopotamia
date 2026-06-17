---
title: Coding Patterns
summary: How code is shaped in this project — small modules, pure metrics, feature plugins, the shared field as integration seam, tunables as params, and macro thresholds derived from their determinants
tags: [patterns, code, architecture, modules, testing]
deps: [doc03.01.02, doc03.01.04, doc02.02]
---

# Coding Patterns

How code is shaped here. These are standing rules: applied to every code change without being asked. They exist because the easy default — one growing module that does everything — is the wrong one for this simulation, which is read, tuned, and verified far more than it is written.

## One concept per module

A module owns one concept and the state and systems that serve it. A file that grows past its concept splits rather than accumulating a second. Modules use the `foo.rs` form until they hold enough to warrant a `foo/` directory with submodules along the concept's seams — the `elk/` split into `movement`, `metabolism`, `spawn`, `color` is the worked example (doc03.01.02). A new responsibility is a new module, not a new section of an existing one.

## Decisions become pure functions

Behaviour that drives a decision is extracted into a pure free function — plain inputs, a scalar or small struct out, no ECS, no `Grid`, no RNG — and pinned with tests next to it. `cross_desire`, `step_water_penalty`, `world_viewport`, and `migration_residual` are the instances. The system calls the function; the function holds the logic. This is what makes a behaviour legible, fast to test, and safe to change: the test pins the *contract*, so the body is swappable. The verification skill is the method; this is the rule that the method produces.

## Feature plugins over a thin root

A feature is a module exposing a Bevy `Plugin` whose `build` registers that feature's resources, systems, and schedule. The composition root stays small — it adds plugins and runs. Simulation plugins carry no rendering dependency so they load headless for tests (doc03.01.02).

## Systems integrate through the field

Simulation systems do not call one another; they read and write a shared lattice of typed per-cell fields on `Grid` — grass, water, poop, water_prox, soil, browse, ford. A system's inputs are the fields it reads and its params; its outputs are the fields it writes. This is the decoupling the field/body split buys (doc03.01.04): the river generator authors `water` and `ford`, and the elk never know a river exists — they read a water cost and a ford mask. The coupling is real and wanted; it just flows through the field, which is the integration seam.

Because the seam is shared, each field is a contract with one producer and several consumers. Before changing a producer, enumerate the consumers and the invariants they assume — `water` and `ford` are read by grass capacity, by browse, *and* by elk movement (`step_water_penalty`, the ford discount), so the river's geometry is, through that field, an input to migration. A field's orientation relative to the migration axis (+x, doc02.02) is a design invariant, not an incidental of whichever system writes it: a producer change that silently flips such an invariant is the failure this rule exists to prevent.

## Tunables are params, not constants

A value the simulation balances around is a field on a params resource with a slider, not a hard-coded constant — so it is visible, adjustable at runtime, and reachable by the balancing harness. A genuinely fixed structural constant stays a `const`; a number that shapes behaviour the player or the sweep tunes is a param.

## Macro thresholds derive from their determinants

A behavioural invariant is pinned by a threshold, and the threshold is expressed relative to whatever determines it — never mirrored from the code or calibrated to one world. A test reads the source-of-truth constant (`EDGE_COL`, `TARGET_POPULATION`) rather than copying its value; a traversal budget scales with the distance to cover (`GRID_WIDTH`); an ecological floor scales with the world's grass ceiling (`Σ capacity`), not a fixed crop count. A magic number in a macro test is a hidden coupling to the one configuration it was written under: change the grid size or a field's layout and the test breaks for the wrong reason. Thresholds start loose and tighten only when a real regression motivates it (doc02.02).
