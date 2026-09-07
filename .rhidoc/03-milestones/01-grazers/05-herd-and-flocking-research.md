---
title: Research: Herd Mechanics & Flocking
summary: Research session — boids' separation/alignment/cohesion adapted from velocity-steering to per-neighbor move-scoring on a lattice; social foraging (local enhancement) as a two-radius forage sense; a time-growing migration drive; pack affiliation as a partition over bodies; and the two-phase snapshot-then-move ECS pattern that scales to hundreds of bodies
tags: [research, exploration, herd, flocking, boids, foraging, migration, ecs, scale]
deps: [doc03.01.01, doc03.01.04]
---

# Research: Herd Mechanics & Flocking

A research session held while building the grazers milestone (doc03.01.01), extending the body side of the field/body split (doc03.01.04). Grass is a field; elk are bodies. This session is about how a population of bodies organizes itself — herds that hold together yet spread out, that chain toward food by watching each other, that drift across the map under a growing pressure, and that do so by the hundreds without the per-tick cost exploding.

## Boids on a lattice: scoring, not steering

The canonical model of collective motion is Craig Reynolds' **boids** — three local rules over a body's neighbors: **separation** (avoid crowding), **alignment** (match neighbors' average heading), **cohesion** (steer toward neighbors' average position). Separation and cohesion in tension produce the signature "spaced but together" texture of a flock.

Reynolds' boids steer a continuous velocity vector. Elk move on a lattice in four discrete steps. The adaptation is to stop tracking velocity and instead **score the candidate moves**: each tick an elk evaluates its four neighbor cells, sums a set of weighted drive terms into a desirability per cell, and picks one. The pick is **weighted-random over the scores**, not a strict maximum — stochastic choice keeps a herd from collapsing into lockstep and lets separation do its work. Each drive is a pull (or push) direction projected onto the four lattice steps; the score is the dot product, scaled by the drive's weight.

## The drives

| Drive | Reads | Radius | Sign |
|---|---|---|---|
| Separation | nearby elk | short | repel |
| Cohesion | herd centroid | medium | attract |
| Alignment | neighbors' last step | medium | attract |
| Local grass | grass on nearby cells | **short** | attract |
| Social foraging | elk currently grazing | **long** | attract |
| Migration | global vector to the far side | — | attract, grows with time |

Alignment is the weakest term on a random-walk lattice, where heading is barely defined; the migration drive already supplies a shared direction, so alignment earns its weight only if travel reads as too jittery without it.

## Two-radius foraging: local enhancement

Social foraging — animals drawn to conspecifics already feeding — is a named ecological mechanism: **local enhancement**. Its modeling value here is the **radius asymmetry**. An elk senses grass itself only at short range, but detects a *grazing* herd-mate at long range. A discovered patch therefore pulls the whole herd in through the bodies already eating it, before the rest can sense the grass directly. The grazers act as long-range beacons for the food they stand on; following each other to food is the emergent result, not a coded rule.

The signal a grazer broadcasts is a body flag (`grazing`), set the tick it eats — the same shape as the `energy` and `digesting` state on the elk body. Hunger closes the loop: scaling the grass and social weights by `1 - energy` makes a sated herd drift while a hungry one veers hard toward food, so the `energy` drive shapes behaviour rather than sitting inert.

## The migration drive

A global directional bias toward the far side of the map, its strength a scalar that grows with elapsed time. Bodies spawn clustered against one edge; cohesion holds them as a herd, the growing migration term drags them across, and foraging detours them through grass patches on the way. The strength is a tunable resource with a UI control, in the manner of the other simulation knobs.

## Packs as a partition over bodies

A population of two hundred is not one herd but many. A **pack** is a partition over the bodies: each elk carries a pack key, and the social drives weight same-pack neighbors more than strangers — strong intra-pack cohesion and alignment, mild inter-pack separation. The result is several distinct herds sharing one field, each holding its own shape while migrating, occasionally mingling at a rich patch and parting again. Packs spawn as separate clusters along the entry edge. The pack key is the same kind of partition the field uses for space (doc03.01.04), applied instead to group identity among bodies.

## Scale: the body cost and the field cost are different problems

A target of two hundred bodies on a much larger grid surfaces two independent costs, and they scale against different quantities.

- **Body cost scales with population.** Naive all-pairs neighbor checks are `O(n²)` per tick — at `n = 200` that is forty thousand distance tests per tick, negligible at a 64 Hz fixed step. All-pairs holds comfortably into the low thousands. Beyond that, the `Grid` is already the cure: bucket bodies into cells and consult only cells within the drive radius, turning `O(n²)` into `O(n·k)` for `k` local neighbors. The spatial index that makes this cheap is the same partition machinery the field rides on — neighbor-by-bucket is the lattice's native query.

- **Field cost scales with grid area, and bites first.** Every per-cell system (growth, fertilize) sweeps the whole field each tick. The current field is small; an area blown up to hundreds of cells on a side is hundreds of thousands of cells swept per system per tick — a far larger number than the body count, and the cost that dominates at scale. Its levers are orthogonal to the body levers: iterate only active or changed cells, coarsen the field's tick relative to the bodies', or partition the field into chunks updated on a rotation.

- **Rendering scales with grid area too.** One sprite per cell does not survive a large grid. The field reads as a single image — a texture whose pixels are cell state — rather than a sprite per cell; bodies remain individual sprites because they are sparse.

The synthesis: bodies are cheap and stay cheap with a spatial index; the **field** — its per-cell sweep and its per-cell rendering — is the quantity that grows with the world and the one a large world must address. This is the scale faultline the network milestones inherit, where the "field" becomes per-node state over a graph and its sweep cost scales with the node count.

## The two-phase move pattern

A body cannot read every other body's position while mutating its own in a single ECS query. Herd movement is therefore two passes: **gather** a snapshot of all bodies (cell, grazing flag, pack) into a plain array, then **decide and write** each body's next cell by scoring its neighbors against that immutable snapshot plus the field and the migration term. The snapshot also fixes the semantics — every body decides against the same frame of the world, rather than reacting to half-updated neighbors.

## Where this leans

- Separation and cohesion are the core; alignment is optional and migration substitutes for much of its directional role.
- Local enhancement lives entirely in the radius gap between sensing grass and sensing grazers — it is one asymmetry, not a subsystem.
- Packs are a partition key on the body, reusing the partition idea the field applies to space.
- The population scales with a spatial index; the field area is the harder cost and the one the larger milestones must solve.

## Open questions

- The right weighting curve between intra-pack and inter-pack social drives — whether packs repel as wholes or only their members do.
- Whether migration strength is uniform across packs or a per-pack trait, giving staggered departures.
- The grid area at which per-cell sprite rendering must yield to field-as-texture, and whether that pressure arrives within the grazers milestone or after it.
- Whether the spatial index for bodies is the existing cell `Grid` reused, or a coarser bucket grid sized to the drive radius.
