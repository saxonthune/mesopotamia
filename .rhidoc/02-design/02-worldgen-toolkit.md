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
  anchor cut (doc03.01.06). Runs 8-connected with diagonal steps weighted by √2, so the path follows
  a warped valley at its true angle instead of quantizing into axis-aligned staircases. Reads: the
  cost field.
- **Anisotropic / directional cost metric** `[in use]` — biases the geodesic downstream while noise
  pulls meanders. Reads: heading and the cost field.
- **Space-colonization algorithm** `[in use]` (shrub raster) / `[in kit]` (river-scale) — grows curved
  dendritic networks (deltas, tributaries, veins, shrub vines) by scattering open-space attractors and
  stepping each growth node toward the average pull of the attractors nearest it, consuming each
  attractor as a node reaches it. The meander is emergent: eating the near attractors swings the
  remaining pull, so branches curve on their own. Open venation (one nearest node per attractor) reads
  as branches; closed venation (an attractor feeds several nodes, dies only when surrounded) reads as
  looped leaf veins. The shrub-tile raster realizes a tile-local variant of this growth (the
  open-space branch growth operator under Derive); river deltas and tributaries remain in the kit.
  Reads: the seed nodes already placed and the attractor set.
- **Recursive subdivision (BSP / k-d) and Voronoi/Delaunay** `[in kit]` — partition space into named
  regions; the natural way to turn the voids the rivers leave into addressable centers.

## Distribute — void-aware spacing

The operators that place discrete features so they spread rather than clump — the genuinely new
machinery the method calls for (doc01.04).

- **Poisson-disk sampling (blue noise)** `[in kit]` — even-but-irregular point sets with a minimum
  spacing. Reads: the points already placed.
- **Mitchell's best-candidate** `[in use]` — cheap blue noise where each pick is the candidate
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
- **Domain warping** `[in use]` — warps noise coordinates by other noise so flat blobs read as
  foliated, flow-like geology; high yield for low cost. The river cost field is warped by two
  broad-wavelength displacement fields before the carve, so the straight least-cost valleys bend
  into sinuous ones and the meander emerges from the field rather than from per-step jitter. Reads:
  the base field.
- **Distance transform / multi-source BFS** `[in use]` — turns distance-to-water into the proximity
  field that soil and vegetation key off (doc03.01.06). **Jump-flooding (JFA)** `[in kit]` is the
  fast variant. Reads: all standing water.
- **Flow accumulation + watershed (D8 / priority-flood)** `[in kit]` — true hydrology: water routes
  downhill, lakes pool in real basins. The fully-causal geology→hydrology path; trades authorial
  control for emergence. Reads: an elevation field.
- **Reaction-diffusion (Turing patterns)** `[in kit]` — organic blotch and patch textures; a flora
  patchiness source if vegetation clumping needs to read more biological. Reads: an initial field.
- **Metaballs (summed radial kernels, thresholded)** `[in use]` — paints an organic filled footprint
  at a placed center: one to three smooth falloff kernels summed into a field and cut at an iso-level.
  One kernel reads round; two offset kernels read as a peanut or an oblong bulge; the same summed
  field doubles as basin depth, deepest at the kernels and shallow at the rim. The shape operator
  paired with best-candidate's positions to give lakes spaced placement and varied, non-circular
  basins. Reads: a placed center and its child seed.
- **Stroke / glyph stamp (parametric curve rasterized)** `[in use]` — paints a discrete curved mark
  at a placed center by walking a parametric curve and stamping a thickness around each sample: a
  sine segment whose half-period reads as a crescent and full period as a tilde, oriented roughly
  horizontal with per-mark jitter on length, amplitude, and angle. The discrete-feature shape
  operator for rough terrain — units placed and spaced (best-candidate) rather than a threshold over
  noise, which only yields patches. Reads: a placed center and its seed.
- **Open-space branch growth (space colonization + distance-transform void fill)** `[in use]` — grows
  a curvy, space-filling vine network at a placed center, the shrub-tile raster. A rough trunk is laid
  corner-to-corner: a spline through jittered, alternating-side waypoints (midpoint-displacement
  roughness, at least two extrema so it bends both ways). Branches are grown, not subdivided — the
  trunk's signed curvature is read, one branch is forced at the strongest bend of each turn-sign so
  both sides always get one, and each branch probes a fan of launch directions and commits to the one
  with the most open space ahead, keeping the parent's momentum and then rounding toward the room (so a
  branch loops backward when that fills its share of the space better). Children branch off children
  for a bounded number of generations. A void-fill pass then closes the gaps the growth left: a
  multi-source BFS distance transform off all the ink finds the emptiest interior pocket and routes a
  curve from the nearest existing curve toward it, repeating until no pocket exceeds a threshold. Every
  stroke is truncated against an occupancy mask, so curves never cross (raster-level constraint
  gating). The bottom-up, space-driven counterpart to top-down subdivision; composes the
  space-colonization, distance-transform, and rejection-gating operators. Reads: a placed center, its
  seed, the parent curve, and the running occupancy and distance state.

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
