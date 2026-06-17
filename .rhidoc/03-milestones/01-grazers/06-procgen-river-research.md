---
title: Research: Procedural River Generation
summary: Research session — procedural generation as a function from seed and a declarative spec to content; the noise-plus-pathfinding river built generate-and-test over constructive carving; value noise via box-blur smoothing as a cost field; directed least-cost (Dijkstra) carving as a geodesic in an anisotropic metric, with heading, drift, and bendiness; several rivers from one shared cost field spaced by a period; rasterizing a centerline to width with a falloff; and a multi-source BFS turning water distance into a grass carrying-capacity field
tags: [research, exploration, procgen, noise, pathfinding, dijkstra, bfs, river, field]
deps: [doc03.01.01, doc03.01.04]
---

# Research: Procedural River Generation

A research session held while building the grazers milestone (doc03.01.01). It authors the water field the rest of the simulation reads: rivers are generated once at startup from a declarative spec, and the distance from that water sets where grass can grow. This is a *field-authoring* step on the Eulerian side of the field/body split (doc03.01.04) — it writes per-cell state into the `Grid` before any body moves, rather than simulating anything over time.

## Procedural generation as a function from seed to content

Procedural generation is, at root, a **pure function from a small seed to a large artifact**. The seed is a single number; the same seed always yields the same world, so a river is reproducible, shareable, and debuggable by its seed alone. The generator's only entropy source is a seeded PRNG (`StdRng::seed_from_u64`), never the OS clock — determinism is the property that makes generated content a stable part of the spec rather than a fresh accident each run. Alongside the seed, the generator takes a **declarative spec** — how many rivers, how far apart, which way they flow, how hard they bend — so the watershed's shape is authored by intent and the noise only fills in the texture between those choices.

Two broad strategies sit under that function:

- **Constructive** — build the artifact so it is valid by construction. A drunkard's-walk river that steps a cursor edge-to-edge is constructive: it can never produce a disconnected channel because it never lifts the pen.
- **Generate-and-test** — produce a candidate field, then run a procedure that extracts a valid result from it. The river here is generate-and-test: noise produces a *terrain of costs* with no river in it at all, and pathfinding then *finds* the river as the cheapest crossing.

The chosen approach is **noise + pathfinding**. Noise supplies the organic, unplanned shape; pathfinding supplies the hard guarantee — a connected channel from one edge to the other — that a pure random walk cannot promise. Each half covers the other's weakness: noise alone has no goal, pathfinding alone has no character.

## Noise as a cost field

The river needs to *meander*, and meandering is the visible trace of a path taking the locally-cheap route through a landscape of varying resistance. So the first artifact is a **cost field**: one scalar per cell, the price of routing the river through it.

White noise — an independent random value per cell — is too sharp; a least-cost path through pure static is jagged, not sinuous. The fix is to **smooth** it. Repeated box-blur passes (each cell averaged with its four neighbors, double-buffered so a pass reads the previous state, not its own half-written output) turn white noise into **value noise**: low-frequency, spatially-correlated variation. The number of smoothing passes is the dial for meander *wavelength* — more passes mean broader, gentler bends; fewer means tighter wiggles. This is where the "drift/momentum" intuition of a random walk maps onto a pathfinding generator: it is not a per-step memory but a **spatial frequency** of the cost terrain. How far a channel is allowed to stray from its heading — its **bendiness** — is a separate dial: the weight of the noise against the directional drift cost introduced below.

Costs are quantized to integers (`(v * 1000) as u32 + 1`) because the pathfinder demands a total order on costs, and `f32` has none — `NaN` makes floats only partially ordered, so they cannot key a priority queue. The `+ 1` floors every cost at one, keeping all moves strictly positive so the search is well-behaved.

## Carving the channel: a directed least-cost path

With a cost field in hand, each river is the **cheapest path** from an entry cell on the top edge to an exit cell on the bottom edge. This is single-source shortest path on a grid graph, solved with **Dijkstra**: a min-priority-queue (`BinaryHeap` inverted with `Reverse` into a min-heap) expands the frontier in order of cumulative cost, recording each cell's predecessor; once the goal is popped, walking the predecessor chain backward reconstructs the centerline.

Dijkstra is the right tool because the cost guarantee is exactly what it provides: the path is **connected by construction** (it is a walk through adjacent cells) and **globally optimal** for the cost field, so the river reads as having found the path of least resistance — which is precisely how real rivers look. A* would add a goal-directed heuristic for speed, but on a grid this size the unguided search is already negligible.

The river flows in a chosen **heading** — top to bottom — with a lateral **drift**: the exit cell sits a fixed fraction of the map's height to the right of the entry, so the channel leans steadily downstream rather than running straight down. Heading and drift are not enforced by clamping the path; they are expressed as an **anisotropic cost** — a step that advances along the heading is cheap, a step that backtracks against it is dear. The river is then a **geodesic in this directed metric**: Dijkstra finds the lowest-resistance descent, and because the metric already leans, the channel leans with it while the noise still pulls it into meanders. The contest between the two — noise amplitude against the directional penalty — is the **bendiness** knob: let the drift dominate and the river runs nearly straight; let the noise dominate and it wanders far off its heading before the drift reels it back.

## Several rivers from one cost field

Multiple rivers share a single cost field and differ only in where they enter. A **period** sets the spacing between entry columns along the top edge, so a count of two drops two channels a fixed distance apart; each carves the same directed-Dijkstra path on the shared terrain. Because they read the same noise, they belong to the same landscape — bending in sympathy where the terrain is soft, holding apart where the period keeps them — rather than looking like two unrelated lines. Shallower, narrower secondary channels are the same carve with a lower rasterized depth and radius.

## From centerline to a river with width

The path is one cell wide — a line, not a river. **Rasterization** gives it body: each centerline cell stamps a disc of water into the surrounding cells, the water level falling off with distance from the center. Two radii shape the cross-section:

- A **core radius** within which water is full-strength, guaranteeing a minimum width regardless of falloff — a core radius of one yields a channel at least three cells across, so the river never pinches to a thread.
- An **outer radius** over which the level tapers linearly to zero, giving soft banks rather than a hard-edged canal.

Stamps **max** into the field rather than overwriting it, so where the river bends back near itself the overlapping discs merge into the higher value instead of the later stamp erasing the earlier. The centerline is the skeleton; the falloff is the flesh.

## Water distance as a carrying-capacity field

The river's purpose is to feed grass, so the last step converts geometry into an ecological constraint: a **grass carrying-capacity field**, one cap per cell, derived from how far that cell sits from water.

The distance is computed with a **multi-source breadth-first search**. Every water cell is seeded into the queue at distance zero simultaneously; BFS then floods outward, and because every frontier edge has unit cost, the first time a cell is reached is its true distance to the *nearest* water — many sources, one pass, no per-cell nearest-water search. The integer ring-distance maps to a capacity: zero inside water (grass cannot grow in the river), full one cell out, tapering linearly to zero at a reach of several cells, and flat zero beyond. Grass far from water is not merely sparse — it is *capped*, unable to grow past its cell's ceiling no matter how much it is fertilized.

This closes the field-authoring chain: noise → cost field → carved centerline → rasterized water → BFS distance → capacity. The simulation's growth systems then read the capacity field and never know a river was involved; they only see that some cells hold more grass than others.

## Where this leans

- Generation is a deterministic function of a seed and a declarative spec; the PRNG is the only entropy and the clock is never touched, so a world is reproducible from the two.
- Noise supplies character, pathfinding supplies the connectivity guarantee, and the anisotropic metric supplies the heading; the river is the seam where a generate-and-test pipeline beats any single part alone.
- Meander has two independent dials — **wavelength** (smoothing passes) and **bendiness** (noise amplitude against the directional drift cost) — not a per-step momentum.
- The spec authors only the *shape* of the watershed (count, period, heading, drift, bendiness); the smoothing passes, endpoints, fords, oxbows, shallow tributaries, and grass capacity all fall out downstream of it.
- Integer costs are a hard requirement of the priority queue, not an optimization — floats cannot key it.
- The pipeline is pure field authoring: a function from (seed, spec, dimensions) to per-cell fields, with the plugin only inserting the result into the `Grid`. It writes once on the Eulerian side of the field/body split, and the running simulation reads the result without re-deriving it.

## Open questions

- Whether the rivers should stay static or evolve — a flowing field that shifts its banks over time would move them from one-shot authoring onto the simulated Eulerian side.
- Whether a branching delta or tributary network wants a space-filling drainage algorithm rather than independent directed paths on the shared cost field.
- The relationship between the capacity reach and the grid scale — whether reach is an absolute cell count or should scale with the world so the green band stays proportionate on a larger map.
- Whether water should also gate body movement (elk avoiding or fording the channel), which would make the river a field the *bodies* read, not only the grass.
