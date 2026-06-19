---
title: ECS for Dummies
summary: A ground-up primer on the ECS the grazers slice runs on — entity/component/system/resource, the table/column/archetype storage, structure-of-arrays vs array-of-structs, how lookups and iteration work, and why the layout pays off
tags: [ecs, storage, architecture, primer, soa, archetype]
deps: [doc03.01.02, doc03.01.04]
---

# ECS for Dummies

A from-scratch primer on the data architecture under the grazers milestone. Where doc03.01.04 names the *modeling* faultline (which state is a field, which is a body), this doc explains the *engine machinery* both forms sit on: what an entity actually is, how components are stored, and why the layout is fast. Read it once and the rest of the codebase — `Query`, `Res`, the hand-built `Grid` — stops being vocabulary and becomes structure.

## The four nouns

ECS is data-oriented: logic lives in functions, state lives in data, and the two are kept apart.

- **Entity** — an **id**, nothing more. An opaque handle that holds no data. It names a thing; it does not contain it.
- **Component** — a typed **piece of data** attached to one entity. *One* value (this elk's `Transform`), not a collection.
- **System** — a **function** whose parameters declare what data it reads and writes. The engine runs it; it is never called by hand.
- **Resource** — a single **global value**, not attached to any entity. The grazers grid is a resource, not a swarm of entities.

The one-line cure for the entity/component mix-up: an entity is the *identity* (an empty id); a component is an *adjective* hung off it; a system is the *verb*. An entity "has" components — it is not "made of" them.

## What an entity is, concretely

An entity id is not a UUID. It is a lightweight value of roughly 64 bits: a **32-bit slot index** plus a **32-bit generation**.

```
Entity { index, generation }
          │       └─ how many times this slot has been reused
          └─ which slot
```

This is the **generational-index** pattern. Despawning an entity frees its slot; the next spawn reuses the slot but bumps the generation. A stale handle held past a despawn carries the old generation, so the engine detects the mismatch and refuses it — a recycled slot can never be silently mistaken for the dead entity that used to live there. Ids are cheap to copy and carry no allocation.

## How components are stored: tables and columns

Components of the same type are stored **together in one contiguous array**. Different component types are **different arrays**. The unit that owns these arrays is a **table**, and each array is a **column**.

```
Table for entities with {Elk, Transform, Sprite}:

  Elk        col:  [ Elk0,  Elk1,  Elk2,  … ]
  Transform  col:  [ T0,    T1,    T2,    … ]
  Sprite     col:  [ S0,    S1,    S2,    … ]
                      ↑      ↑      ↑
                    row0   row1   row2     ← each row is one entity
```

One column per component type; rows aligned by entity. **The column is all of them; a component is one of them.** Calling the array "a component" re-introduces exactly the confusion the layout avoids.

## Archetype: the set of component types

An **archetype** is a unique *set* of component types — the identity of a table. Every entity holding exactly `{Elk, Transform, Sprite}` is filed in one archetype; an entity holding `{Elk, Transform, Sprite, Grazing}` is a **different** archetype with a **different** table. Adding or removing a component changes an entity's archetype, so the engine copies its row to the new table. Frequent add/remove is therefore not free — it is a table move — which is why a component that flips on and off constantly is a candidate for sparse-set storage instead of the default table.

## SoA vs AoS — the reason it is fast

Picture a spreadsheet and ask which direction is contiguous in memory.

**Array-of-structs (AoS)** — each *row* is one contiguous blob:

```
[ {grass:.5, water:0, prox:1} ][ {grass:.2, water:0, prox:.8} ] …
   └────────── cell 0 ──────┘    └────────── cell 1 ──────┘
```

**Structure-of-arrays (SoA)** — each *column* is one contiguous blob:

```
grass: [ .5,  .2,  0,  … ]
water: [  0,   0,  1,  … ]
prox:  [  1,  .8,  0,  … ]
         ↑    ↑    ↑
       cell0 cell1 cell2
```

A "logical record" no longer exists as one object in SoA — it is a row reconstructed by reading index `N` out of every column.

The payoff: a system that touches only `grass` streams a cache line full of *grass it will use*. In AoS the same loop drags in the `water`/`prox` bytes wedged between each grass value and immediately discards them. SoA stores together what is *processed* together, not what is *conceptually* one thing — and it vectorizes (SIMD) for the same reason. Tables-as-columns is SoA; the engine's storage and a hand-built field resource use the identical trick.

## How a lookup works

Finding one entity's component is **not** a hash from the id into the column. It is a two-step indirection:

```
entity.index ─► [ entity-location table ] ─► (archetype/table, row)
                                                      │
                                                      ▼
                              table.column::<Transform>()[row] ─► the component
```

A flat metadata array, indexed by `entity.index`, stores each entity's **location** (which table, which row) and its current generation. The column is then indexed by **row**, never by the id directly. The cost is O(1) — a couple of array indexings — but it is *random* access.

**Iteration is the fast path.** `for (a, b) in &query` does not touch the location table at all: it knows it is marching down matching columns, so it streams rows sequentially across cache-friendly memory. A by-id fetch (`query.get(entity)`) is cheap but random; bulk iteration is cheap *and* sequential. The architecture is tuned for the second.

## When a whole entity is assembled at once

Almost never in hot logic — each system reconstructs only the *subset* of columns it queried. A full assembly of every component happens only in cold paths: a table move (copy all existing columns to the new table), despawn (drop every column for that row), clone, scene serialization, or an inspector. The layout optimizes the common case (a system over a few component types across many entities) and pays a small contiguous-copy cost on the rare case (all components of one entity).

## Where this project sits

The grazers slice runs both sides of the engine deliberately (see doc03.01.04 for the modeling rationale):

- **Elk are bodies → entities.** Sparse, mobile, individual. They live in tables as `(Elk, Transform, Sprite, …)` columns and are reached through `Query`.
- **Grass and the rest of the terrain are a field → a resource.** Dense, anonymous, fixed. The `Grid` is a single `Resource` holding parallel `Vec<f32>` columns (`grass`, `water`, `water_prox`, `soil`, …) — **hand-rolled SoA, indexed by cell number** instead of by entity id. A cell's full state is the tuple of `column[N]` across every array, reconstructed on demand, not stored as one struct.

So the two storage shapes are the same columnar SoA layout with different keys: the engine's tables are keyed by entity (reached via the location table), and the `Grid` is keyed by `row * width + col`. Adjacency on the lattice is arithmetic (`±1`, `±width`), so the grid needs no stored neighbor structure. Rendering is where the two meet: per-cell sprite and digit *entities* carry only an `index`, and the render systems read the `Grid` *resource* at that index to paint them — bodies reading a field.
