---
title: Grazers
summary: First milestone — a 2D grid ECS where grass grows and elk graze; the simplest slice that proves the simulation loop, and the code structure it establishes
tags: [milestone, ecs, grid, simulation]
deps: [doc01.01]
---

# Grazers

The grazers milestone is a self-contained ECS simulation on a 2D grid: grass grows in random cells, and elk wander the grid and eat it. It is the simplest slice that proves the core loop — a stepped world, entities with state, and systems that mutate it each tick — running on screen.

This section holds both what the slice does and how its code is laid out:

- **doc03.01.01 Grazers** — the simulation spec: the world, the elk, and the tick that drives them.
- **doc03.01.02 Structure** — the crate and module layout the slice establishes, which later milestones extend.

It is built in sandwich-programming sessions, in Rust on a Bevy ECS (doc01.01).
