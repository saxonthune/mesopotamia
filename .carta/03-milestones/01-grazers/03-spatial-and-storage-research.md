---
title: Research: Spatial Representation & Storage
summary: Research session during the grazers milestone — spatial representation (grid vs. continuous vs. graph), RimWorld/DF actor routing, ECS storage, network graphs, macro-variable aggregation, and an algorithm inventory for the simulation
tags: [research, exploration, spatial, storage, ecs, architecture]
deps: [doc03.01.01]
---

# Research: Spatial Representation & Storage

A research session held while building the grazers milestone (doc03.01.01). It surveys the option space for how the simulation represents space, routes actors, and stores evolving state — the questions that surface once grass and elk grow into people, networks, irrigation, and trade. Nothing here is a decision; it is the vocabulary and the technique landscape, captured so later work does not re-derive it. Settled choices belong in an ADR; buildable slices belong in a milestone spec.

## The framing: not one decision

"Grid vs. floating-point coordinates" is not a single global choice. Grass and elk are a *spatial/ecological* problem; people-networks, irrigation, and trade are *network/flow* problems. These want different representations, coupled at their seams. The leaning that emerges is to **layer representations** rather than force one model on the whole simulation.

A second framing runs underneath: **simulation state is the source of truth; the screen is a projection of it.** The model (grid cells, entity positions) is what rules reason about; rendering is a separate system that reads state and draws a picture, never the reverse. The mapping from model-space to screen-space is a function the renderer owns — which is why the same state can be drawn as sprites, tints, or raw numbers without touching simulation logic. A Bevy subtlety: an entity's logical position (model) and its `Transform` (view) are distinct, and a render system syncs the second from the first.

## The shared primitive: the neighbor query

Most rules reduce to one operation: *given X, what is near X, and in what state?* Grass growth, an elk sensing grass, water spreading down a channel, a trader finding a market — all gather relevant neighbors and compute a new state. The representations differ mainly in **how they answer "what is near?"**, which decomposes into three semi-independent choices:

1. **What holds the state** — a 2D array of cells, a list of entities with positions, or graph nodes.
2. **How neighbors are found** — index arithmetic, distance math plus an acceleration structure, or graph adjacency.
3. **The update rule** — synchronous cellular-automaton sweep, per-agent asynchronous step, or field diffusion.

## Grid vs. continuous: the calculation difference

**Grid (discrete / cellular automaton).** Neighbors are implicit in memory layout: cell `(r,c)`'s neighbors are `(r±1, c±1)` — integer index math, O(1), no distance computation. "Near" is defined by the baked-in topology (4- or 8-neighborhood, radius-in-cells). A neighbor-dependent growth rule reads the *current* grid and writes a *new* one (double-buffering) so results don't depend on iteration order. Cheap, deterministic, easy to reason about; the cost is quantized space and distorted diagonals. The grazers spec uses the degenerate case — each empty cell grows with a fixed probability, independent of neighbors.

**Continuous (floating-point patches).** Neighbors require distance computation; "near" is a radius. Naive all-pairs is O(n²), so a spatial acceleration structure is needed at scale. Growth becomes a spreading point process (spawn near existing patches) or a density field. The cost is more code and fuzzier "cell state"; the gain is arbitrary placement and smooth spread.

The tradeoff in one line: a grid *pre-computes* the neighbor relationship and stores it in the array's shape; a continuous system *computes* it at runtime.

## How RimWorld & Dwarf Fortress route actors

Both are grid-based and use **A\*** at the core (per-cell movement cost), but the engineering is in what surrounds A\*:

- **Dwarf Fortress — connectivity IDs.** Every tile is labeled with a connected-component id via flood fill. Before running A\*, check whether start and goal share an id; if not, no path exists — reject in O(1) without searching. Recompute affected components when terrain changes. This is the single biggest optimization for the "path that doesn't exist" worst case.
- **RimWorld — region system.** The map is divided into small contiguous **regions** forming a graph. Reachability ("can this pawn get to X?") is a cached BFS over that graph, not a full-map A\*. Each region also keeps **lists of the things inside it**, so "find the nearest haulable" traverses regions outward and checks their lists — never scanning the whole map.

A key separation: **pathfinding and target-selection are different problems.** A\* answers "how do I get there"; the region/list structure answers "where is the nearest thing worth going to." An elk detecting grass is the *second* problem — answered cheaply by a spatial index queried outward.

Two techniques for many agents:
- **Dijkstra maps / flow fields** — compute distance-to-goal for every cell once (a Dijkstra sweep from the goal), then each agent steps downhill. Far cheaper than per-agent A\* when many agents share a goal.
- **Influence maps** — scalar fields over the grid ("danger here", "food density here", "reach of a patron's influence"). Agents decide by sampling the field rather than reasoning about individuals.

And the enabler for scale: **agents do not all think every tick.** RimWorld pawns re-evaluate only when a job ends; expensive scans are staggered; DF abstracts off-site activity. Simulation level-of-detail and tick budgeting are essential once many actors exist.

## Storing evolving state

ECS is itself the answer to "store it so I can pull and write it efficiently" — a data-oriented database for many heterogeneous entities:

- **Columnar (structure-of-arrays) storage.** Entities with the same component set form archetypes; each component type is a contiguous array. A system reading `(Position, Hunger)` iterates tight, cache-friendly arrays — the mechanical reason ECS scales where a list of fat objects would thrash cache.
- **Sparse, heterogeneous attributes are free.** Attach only the components that apply; queries match on component *sets*. No null fields on entities that lack a property.
- **Individual vs. tool maps directly.** Knowledge of smelting is a component on the *person*; tool quality is a component on the *tool*; the efficiency of an action is a function reading both at the point of use — not pre-baked anywhere.

The two parts ECS does not hand over for free:

**Networks (patronage, debt bondage, administration, trade).** These are graphs between entities. Options by weight: store the other party's `Entity` id in a component (`DebtBond { creditor, amount }`) for cheap directional links; a dedicated graph **resource** (adjacency lists) when whole-network queries are needed (who is in a patron's pyramid, cycles in debt); Bevy's native entity relationships for hierarchy-shaped cases. Graph algorithms apply: BFS/DFS, shortest path, cycle detection, topological sort, connected components.

**Macro variables (total debt, food supply, instrument sophistication).** The decision is **derive-on-read vs. maintain-incrementally**: recompute by summing over entities (simple, always correct, O(n) per tick) versus keeping a running total adjusted on change (cheap reads, but a cache-invalidation problem). The common middle path recomputes aggregates on a *slow* tick, since macro values need not be frame-accurate.

**Development over time (irrigation, mining, tech).** Two flavors: world-state structures that grow (irrigation/mine networks — graph/grid resources mutated by construction systems) and abstract capability (accumulated knowledge — scalar/level state on a resource or institution). If a *timeline* of how the world developed is wanted, that is event sourcing — append a log of changes — which also serves save/replay.

## Algorithm & technique inventory

Grouped by the problem solved:

- **Movement:** A\*, Dijkstra; hierarchical pathfinding (HPA\*) and region graphs for scale; flow fields / Dijkstra maps for many-agents-one-goal.
- **Reachability:** connected components / union-find to reject impossible paths in O(1); recompute on terrain change.
- **Spatial queries:** uniform spatial hash, quadtree / k-d tree, or RimWorld-style region + object-lists.
- **Decision-making at scale:** influence maps / scalar fields; utility AI or job-queue systems (RimWorld's WorkGiver → Job → Toil state machines).
- **Storage:** ECS archetype/columnar storage; components vs. resources; `Entity`-id references and adjacency lists for graphs.
- **Graph algorithms:** BFS/DFS, shortest path, cycle detection (debt cycles), topological sort, connected components, community detection.
- **Aggregation:** incremental maintenance vs. periodic recompute; dirty flags; event-driven accumulators.
- **Scheduling / performance:** tick budgeting, staggered updates, simulation level-of-detail.
- **Persistence / history:** ECS world serialization for save/load; event sourcing for a development timeline.

## Spatial representation menu

From most-discrete to most-relational: regular grid (CA); hex grid (equidistant neighbors, no diagonal distortion); continuous + brute force (small agent counts); continuous + spatial hashing (the pragmatic hybrid); trees (quadtree / k-d tree / BVH) for uneven density; Voronoi / irregular polygons (terrain, territory, basins); graph / network (trade, kinship, irrigation — not embedded in metric space); field / continuum (PDE on a grid for water, nutrients).

## Where this leans

- **Ecological/spatial layer** (grass, terrain, elk) → a grid, matching the grazers spec; elk could later carry continuous positions over grid terrain with spatial hashing.
- **Socioeconomic layer** (people, trade, irrigation) → graph / flow networks, built as their own resources/components when those milestones arrive.
- The two couple at interfaces (a settlement node sits at a grid cell) without either being subordinate. ECS makes this comfortable.

## Open questions

- Whether elk movement stays grid-stepped or goes continuous, and at which milestone that matters.
- Where the grid/graph interface lives once settlements and irrigation exist.
- Whether the world needs a development *timeline* (event sourcing) or only current state.
- The trigger point at which macro-variable recompute becomes costly enough to maintain incrementally.
