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

## The Balancing Goal

The demo is a balancing game. Elk movement is driven by two kinds of force. The **natural drives** — separation, cohesion, grass-seeking, and social foraging — are each a response to what an elk senses around it. The **migration pull** is a single uniform pressure toward the far edge that grows over time regardless of local conditions; it is the artificial hand on the scale, moving the herd even when nothing in the world would.

The demo surfaces, for any moment, how much of a herd's movement comes from the natural drives and how much from the migration pull. The player tunes the sliders — drive weights, perception radii, metabolism — toward a settling where the herd travels on its natural drives while staying fed, so the migration pull can recede and the herd still moves and survives on its own.

To make that tuning possible the demo exposes a readout and a control for each force in play, arranged so a change and its consequence are both legible: adjusting a weight visibly shifts both the force balance and the survival outcome.
