---
title: Goal
summary: The hotspot milestone's destination — a rendered ocean-and-sky scene where islands build up over a fixed volcanic source and melt away as they drift off it, with vegetation that co-evolves with the land and jumps from a dying island to a young one
tags: [milestone, rendering, procgen, lifecycle, co-evolution]
deps: [doc01.03]
---

# Goal

## The inspiration

The Hawaiian islands form over a **hot spot** — a fixed plume of magma in the mantle. The Pacific plate drifts steadily across it in one direction. While a patch of crust sits over the plume it is fed from below and grows into an island; as the plate carries that island off the plume the feed stops, and the island erodes and subsides back beneath the sea. The hot spot stays put while the land moves over it, so the islands form a chain: youngest and largest above the source, older and smaller trailing away, the oldest worn back down to nothing. Each island is the same lifecycle caught at a different age.

This milestone renders that lifecycle. A piece of land is not a static object but something born over the source, raised while it stays there, and dissolved once the drift takes it away — a cycle in space rather than a structure in place.

## What the scene shows

The camera is centered on the hot spot, but the hot spot itself is below the water and out of view; what fills the frame is **ocean and sky**. The ocean is a curved plane — a control sets its curvature, so the horizon can be tuned from flat to strongly rounded.

Across this plane the island lifecycle plays out as an animation:

- An island **builds up** over the hot spot. Its mass accretes irregularly — the buildup is randomized, so no two islands take the same shape.
- The drift carries the island off the source, and it **melts away** uniformly, subsiding back into the ocean as a new island begins to rise behind it.

The result is a moving chain: land emerging at the source, maturing as it drifts, and eroding back to sea, continuously.

## The land and its vegetation co-evolve

A second layer rides on the first. An island begins **rocky** and bare. As it matures, **vegetation appears** on it. When the island reaches the end of its life and a new one is forming, the vegetation does not simply die with it — it **jumps from the mature island to the young one**, the way life colonizes a fresh island from an older neighbor.

The foliage is itself generated, not fixed: its details are produced randomly, and they keep changing — drifting as an island matures and shifting again as life crosses from one island to the next. Land and vegetation each shape the other across the jump: the form of the island sets where life can take hold, and the life carried forward is the seed the next island inherits. This is the co-evolution of content and form (doc01.03) made watchable — the same dialectic of parts remaking the whole that shaped them, here staged as foliage and island remaking each other across the lifecycle.

## What it demonstrates

The milestone is reached when the scene runs on its own: a curved ocean under sky, islands rising over the hot spot and dissolving as they drift, and vegetation greening the mature land and leaping to the next island as the chain renews itself.
