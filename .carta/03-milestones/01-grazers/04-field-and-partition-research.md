---
title: Research: Field vs. Body & the Grid→Graph Bridge
summary: Research session — the Eulerian/Lagrangian field-vs-body faultline under three vocabularies, the field/body decision rule, bodies-over-a-field as one engine, and how milestone-1's grid is the same Eulerian-field-over-a-partition pattern the DESIGN.md graph models use with adjacency swapped from arithmetic to a matrix
tags: [research, exploration, spatial, eulerian, lagrangian, partition, graph, ecs]
deps: [doc03.01.01, doc03.01.03]
---

# Research: Field vs. Body & the Grid→Graph Bridge

A research session held while building the grazers milestone (doc03.01.01). Where doc03.01.03 lays out the *option menu* — grid vs. continuous, the neighbor-query primitive, ECS storage, the algorithm inventory — this session names the *structure underneath those options* and follows it to its conclusion: the same pattern that represents grass and elk on a grid is the pattern the `DESIGN.md` debt and ecology models run on a graph. Nothing here is a decision; it is the conceptual spine, captured so the jump from the grazers grid to the network models does not feel like a new architecture.

## The faultline: field vs. body

Every piece of simulation state lives in one of two forms, and the choice is the deepest modeling decision in the project:

- A **body** carries its own state, position included. You ride along with it. State lives *on the thing*.
- A **field** is state pinned to fixed locations; you stand at a location and read what is there. Position is not a variable, it is an index. State lives *at the place*.

Grass is a field — every site of ground has a vegetation state, immobile, anonymous, defined at its location. Elk are bodies — sparse, mobile, individual, their position a property they carry.

This single line runs under three vocabularies, which are the same distinction renamed by discipline:

| Discipline | "Body" | "Field" |
|---|---|---|
| Numerical physics | Lagrangian (track particles) | Eulerian (watch fixed cells) |
| Complex-systems modeling | Agent-Based Model | Cellular Automaton |
| Game engine / ECS | entity | a spatial index (the grid) |

Agent-based modeling subsumes cellular automata: a cellular automaton is the special case of agents frozen into a lattice and stripped of identity. Promoting grass from "blade" to "cell state" is exactly that move.

## The decision rule

Whether a given organism is a field or a body is not abstract — it answers two concrete questions:

1. **Does it have individual identity or behavior worth expressing?**
2. **Does it occupy one site or many?**

| Organism | Identity? | Sites | Form |
|---|---|---|---|
| Grass, moss, flower | no | one | **field** — a cell state |
| Fern, small bush | maybe | one | borderline — modeler's call |
| Tree, large shrub | yes | many | **body** — an entity over the grid |
| Elk | yes | one (mobile) | **body** — an entity over the grid |

Dense + anonymous + single-site → field. Sparse + individual + multi-site (or mobile) → body. The fern sits on the seam: a cell state until a fern needs to *do* something individual, then a body.

## Bodies over a field: one engine

A body and a field interact in one direction — the body reaches down and mutates the field beneath its footprint:

- An elk writes `Empty` to the one grass cell it stands on.
- A tree writes "shaded" to the dozen cells under its canopy, suppressing grass growth there.

These are the same operation at different footprint sizes. A tree is not a larger cell; it is a stationary elk that shades instead of eats. The grid never has to be made big enough to "hold" a tree, because the tree is never stored in a cell — it is a body layered over the field, touching the cells it overlaps. The multi-scale problem dissolves the moment the tree stops being a field value.

This is a named numerical pattern, not a contrivance: hybrid particle/grid methods (PIC/FLIP, the Material Point Method) carry state on Lagrangian particles and resolve interactions on an Eulerian grid, transferring between the two each step. "Bodies over a field" is that architecture, arrived at from the grazers slice rather than from fluid dynamics.

## Two partitions in ECS, orthogonal

A subtlety surfaces once the field lives inside Bevy: two independent partitions are in play, and conflating them causes confusion.

- **The storage partition** is the engine's, and it is about cache locality. Bevy groups entity *data* into archetypes by component-set; components are contiguous columns. This partition concerns *what components an entity has* — it has nothing to do with space (see doc03.01.03 on columnar storage).
- **The domain partition** is the modeler's, and it is about the world. The grass grid is a map from a *spatial* key to state.

ECS is deliberately strong at the first and weak at the second — querying and iterating entities is its core competence; spatial structures are not. So the `Grid` resource is a hand-built spatial index bolted onto an ECS whose native partition is orthogonal to space. This is the correct shape, not a workaround: the engine partitions by type, the model partitions by place, and the two coexist without interfering.

## The partition need not be Euclidean

A grid cell is a special case of a general object: **a bucket of a partition holding aggregate state, addressed by a key.** Euclidean position is only the most obvious key. The same field machinery runs over any partition:

| Partition key | A "cell" is | Adjacency is | Suits |
|---|---|---|---|
| `(x, y)` lattice | a tile | the 4/8-neighborhood | grass, terrain |
| hash bucket | a hash cell | bucket membership | unbounded / sparse space |
| quadtree node | an adaptive region | tree parent/children | mixed-density worlds |
| **graph node** | **a bank / a compartment** | **the edge / flow matrix** | **debt, ecology** |

Choosing the partition is choosing which queries are cheap and what detail is blurred away inside a bucket. A uniform lattice is the partition where this choice is invisible because every cell is identical and neighbors are arithmetic.

## The conclusion: the grid is the graph models with adjacency swapped

The cost of adjacency is the seam, and it differs by partition:

- On a **lattice** (the grazers milestone), adjacency is *computed*: the neighbors of `(x, y)` are `(x±1, y±1)`. Nothing is stored; the grid's array shape *is* the adjacency, supplied for free. No separate "nearness" structure is needed at this milestone — an elk reads the grass field at its own index, and movement reads the four neighbor indices.
- On a **graph** (the `DESIGN.md` models), adjacency is *arbitrary* and cannot be derived from coordinates — bank A owes bank C but not B. It must be stored explicitly. That stored adjacency is precisely the DebtRank liabilities matrix `L` and the Ulanowicz flow matrix `F`. There the adjacency structure is not a parallel index; it is the centerpiece.

So the endgame models are **Eulerian fields over a non-Euclidean partition**: state lives in the cells of a graph (equity per bank, biomass per compartment), and the neighborhood is a matrix rather than a lattice. A shock propagating bank→bank is a perturbation moving through that field — structurally the same as an elk thinning grass, with adjacency defined by `L` instead of by integer offsets.

The grazers grid is therefore not a detour from the network models — it is those models with the hardest part (arbitrary adjacency) made invisible by a uniform partition. Learning to think "a field of state over a partition, with bodies moving across it" on the lattice is the exact muscle the debt and ecology engine needs once the partition becomes a weighted graph.

## Where this leans

- The grazers milestone occupies the cheapest corner of the whole space: grass as a Eulerian field on a lattice (adjacency free), elk as Lagrangian bodies (a handful, no spatial index warranted).
- Larger flora (trees) enter as bodies over the field, not as larger cells — the "bodies over a field" pattern already in use for elk.
- The transition to the `DESIGN.md` models is a change of partition (lattice → weighted graph) and of where adjacency is stored (computed → matrix), not a change of architecture.

## Open questions

- The point at which a borderline organism (fern, bush) earns promotion from cell state to body.
- Where the lattice/graph interface lives once a settlement node sits at a grid cell and also participates in a trade network.
- Whether the elk ever leave the lattice for continuous positions over grid terrain, and at which milestone that pressure arrives (echoes doc03.01.03's open question on grid-stepped vs. continuous movement).
