---
title: Worldgen Toolkit
summary: The catalogue of CS/algorithm operators that actualize the unfolding method — each one a structure-preserving transformation over the grid state, grouped by the role it plays in the generative sequence, tagged by whether the code calls it or it merely waits in the kit
tags: [worldgen, generation, algorithms, operators, toolkit, procgen]
deps: [doc01.04, doc03.01.06]
---

# Worldgen Toolkit

Every tool here is an **operator over the grid state**: it takes the wholeness as it currently exists
and returns a richer wholeness, structure-preservingly. The unfolding method (doc01.04) is the
composition of these operators in a backtrack-free sequence, so the catalogue is grouped not by data
structure but by the *role in the unfolding* each operator performs.

**The binding rule.** An operator earns its place by naming two things: the structure-preserving job
it does, and the **read-set** it consumes — which already-placed centers it reads. An operator whose
read-set is a global field computed once is suspect; an operator whose read-set is the current state
is the method working. This rule is what stops a powerful tool from being reached for merely because
it is powerful.

Status is a present fact about the code, not a plan: **`[in use]`** means the pipeline calls it;
**`[in kit]`** means it is catalogued and available, drawn on when a layer needs it.

## Subdivide — make the differentiating cuts

The operators that cut the unity into nested regions.

- **A\* / Dijkstra over a cost field** `[in use]` — carves the river as a least-action geodesic; the
  anchor cut (doc03.01.06). Reads: the cost field.
- **Anisotropic / directional cost metric** `[in use]` — biases the geodesic downstream while noise
  pulls meanders. Reads: heading and the cost field.
- **Space-colonization algorithm** `[in kit]` — grows dendritic branching (deltas, tributaries,
  veins) toward scattered attractors. Reads: the trunk already carved and the attractor set.
- **Recursive subdivision (BSP / k-d) and Voronoi/Delaunay** `[in kit]` — partition space into named
  regions; the natural way to turn the voids the rivers leave into addressable centers.

## Distribute — void-aware spacing

The operators that place discrete features so they spread rather than clump — the genuinely new
machinery the method calls for (doc01.04).

- **Poisson-disk sampling (blue noise)** `[in kit]` — even-but-irregular point sets with a minimum
  spacing. Reads: the points already placed.
- **Mitchell's best-candidate** `[in kit]` — cheap blue noise where each pick is the candidate
  farthest from existing features; blue-noise *and* current-state-aware in one operator, the closest
  fit to the void-aware placement the method describes. Reads: the features already placed.
- **Lloyd relaxation (centroidal Voronoi)** `[in kit]` — relaxes a clumped point set toward even
  spread; the operator that un-clumps an existing distribution. Reads: the current points and their
  Voronoi cells.

## Derive — continuous causal fields

The operators that produce field layers as functions of the centers beneath them. Never authored as
independent noise; always derived.

- **Value / Perlin / Simplex noise, fBm** `[in use]` — the texture source feeding the cost field and
  capacity fields. Reads: a seed.
- **Domain warping** `[in kit]` — warps noise coordinates by other noise so flat blobs read as
  foliated, flow-like geology; high yield for low cost. Reads: the base field.
- **Distance transform / multi-source BFS** `[in use]` — turns distance-to-water into the proximity
  field that soil and vegetation key off (doc03.01.06). **Jump-flooding (JFA)** `[in kit]` is the
  fast variant. Reads: all standing water.
- **Flow accumulation + watershed (D8 / priority-flood)** `[in kit]` — true hydrology: water routes
  downhill, lakes pool in real basins. The fully-causal geology→hydrology path; trades authorial
  control for emergence. Reads: an elevation field.
- **Reaction-diffusion (Turing patterns)** `[in kit]` — organic blotch and patch textures; a flora
  patchiness source if vegetation clumping needs to read more biological. Reads: an initial field.
- **Metaballs (summed radial kernels, thresholded)** `[in kit]` — paints an organic filled footprint
  at a placed center: one to three smooth falloff kernels summed into a field and cut at an iso-level.
  One kernel reads round; two offset kernels read as a peanut or an oblong bulge; the same summed
  field doubles as basin depth, deepest at the kernels and shallow at the rim. The shape operator
  paired with best-candidate's positions to give lakes spaced placement and varied, non-circular
  basins. Reads: a placed center and its child seed.

## Orient — vector fields

The operators that give a layer direction — the machinery behind the directional grain.

- **Gradient of a scalar field (∇ proximity)** `[in kit]` — yields direction-toward-water for free;
  the bank-parallel grain orientation is its perpendicular. Reads: the proximity field.
- **Curl noise** `[in kit]` — divergence-free flow noise; a wind field that reads as wind. Reads: a
  seed.
- **Line integral convolution (LIC)** `[in kit]` — smears a texture along a vector field into
  streaks; the grain *rendered* directly from the wind/water field. Reads: a vector field.
- **Streamline tracing** `[in kit]` — walks a vector field to place discrete oriented marks. Reads: a
  vector field.

## Segment — know the voids and their adjacency

The operators that let the code see the regions a cut leaves, so the next feature can be placed one
per void.

- **Connected-component labeling (flood fill)** `[in kit]` — turns "the space between rivers" into
  discrete, countable regions. The other half of un-clumping placement. Reads: the carved water.
- **Region adjacency graph** `[in kit]` — lets a feature reason about which void it is in and who its
  neighbors are. Reads: the labeled regions.

## Constrain — make features respect each other

The operators that enforce coherence between neighboring features.

- **Rejection sampling / constraint gating** `[in use]` — accept/reject against the current state;
  already implicit in feature placement, worth naming as an operator. Reads: the current state.
- **Wave Function Collapse / model synthesis** `[in kit]` — constraint-based neighbor-respecting tile
  placement. Strong, but it fights a field-based world; fit only for a tile-discrete layer, not a
  default. Reads: a tile adjacency rule set and placed neighbors.

## Navigate — fauna

The operators the animals use to move through the finished world — the last step in the sequence.

- **A\* and softmax / weighted stochastic choice** `[in use]` — per-agent pathing and the
  score-then-sample step selection (doc03.01.06). Reads: the terrain cost and the agent's drives.
- **Flow-field / vector-field pathfinding** `[in kit]` — one field computed once, every agent
  follows; the herd-scale upgrade to per-agent search. Reads: a goal and the terrain.
- **Steering behaviors / influence maps** `[in kit]` — flocking, avoidance, and gradient-following as
  cheap local rules. Reads: neighbors and a scalar influence field.

## Seed — determinism

The cross-cutting infrastructure that makes every operator reproducible and independently testable.

- **Hierarchical seeded RNG / hash-on-coordinate (PCG, splitmix)** `[in use]` — a master seed splits
  into independent child streams per feature, so each operator is deterministic and pinnable. Reads:
  the master seed and a stream index.
