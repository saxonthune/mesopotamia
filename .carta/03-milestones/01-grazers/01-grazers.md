---
title: Grazers
summary: First milestone — a 2D grid ECS where grass grows and elk graze; the simplest slice that proves the simulation loop
tags: [milestone, ecs, grid, simulation]
deps: [doc01.01]
---

# Grazers

The grazers milestone is a self-contained ECS simulation on a 2D grid: grass grows in random cells, and elk wander the grid and eat it. It is the simplest slice that proves the core loop — a stepped world, entities with state, and systems that mutate it each tick — running on screen. It is built in sandwich-programming sessions, in Rust on a Bevy-style ECS (doc01.01).

## The World

The world is a 2D grid held as a resource. Each cell carries one state:

- **Empty** — bare ground.
- **Grass** — grazeable vegetation in the cell.

Grass appears stochastically: each empty cell has a small chance per tick to grow grass.

## The Elk

Elk are entities placed on the grid. Each elk:

- occupies a cell,
- steps to an adjacent cell each tick (a simple walk),
- eats the grass in a cell it occupies, returning that cell to empty.

Elk render as sprites; grass renders as a sprite or a cell tint.

## The Tick

Each simulation tick runs three systems in order:

1. **Growth** — empty cells may become grass.
2. **Movement** — each elk steps to a neighboring cell.
3. **Grazing** — an elk standing on a grass cell consumes it, clearing the cell.

## ECS Shape

The slice exercises the entity-component-system pattern that later milestones extend:

- **Grid** — a resource storing each cell's state.
- **Entities** — elk.
- **Components** — grid position, sprite.
- **Systems** — growth (writes the grid), movement (updates positions), grazing (an elk clears the grass beneath it).

## What It Demonstrates

The milestone is reached when grass visibly spreads across empty cells while roaming elk thin it back — a graze-and-regrow balance playing out on the grid in real time.
