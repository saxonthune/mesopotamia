---
title: Hotspot
summary: Second milestone — a rendered ocean-and-sky scene centered on a volcanic hot spot, where islands build up over the source and melt away as they drift off it, carrying their vegetation forward to the next island
tags: [milestone, rendering, procgen, lifecycle, co-evolution]
deps: [doc01.03]
---

# Hotspot

The hotspot milestone is a rendered scene rather than a grid simulation. The camera sits over a volcanic hot spot looking out across ocean and sky; islands rise over the source, grow while they sit above it, and melt away once the drift carries them off. It is the project's first piece of continuous-space rendering, and its first study of a lifecycle that is born, matures, and dies in motion.

This section holds the milestone's destination and the slices that reach it:

- **doc03.02.01 Goal** — the inspiration (Hawaiian island formation over a fixed mantle hot spot) and what the scene shows: ocean and sky, islands generated and dissolved, and the vegetation that co-evolves with the land and jumps from a dying island to a young one.

It is built in sandwich-programming sessions, in Rust on a Bevy renderer. The island-and-foliage lifecycle is a concrete staging of the co-evolution of content and form (doc01.03).
