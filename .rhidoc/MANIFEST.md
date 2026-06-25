# .rhidoc/ Manifest

Machine-readable index for AI navigation. Read this file first, then open only the docs relevant to your query.

**Retrieval strategy:** See doc00.00 (codex index) for how to find and read docs efficiently.

## Column Definitions

- **Ref**: Cross-reference ID (`docXX.YY.ZZ`)
- **File**: Path relative to title directory
- **Summary**: One-line description for semantic matching
- **Tags**: Keywords for file-path→doc mapping
- **Deps**: Doc refs to check when this doc changes
- **Refs**: Reverse deps — docs that list this one in their Deps (computed automatically)
- **Attachments**: Non-md files sharing the doc's numeric prefix. Sidecar artifacts that travel with the doc during structural operations. Purely filesystem-derived; not a frontmatter field.

Orphaned attachments (non-md files with no corresponding root .md) are reported as warnings on stderr during regeneration and do not appear in this table.

## 00-codex — Codex

| Ref | File | Summary | Tags | Deps | Refs | Attachments |
|-----|------|---------|------|------|------|-------------|

| doc00.00 | `00-index.md` | Meta-documentation — how to read this workspace | index, meta | — | — | — |
| doc00.01 | `01-about.md` | Why this workspace exists, how to read it, two-sources-of-truth theory | docs, meta, theory | — | — | — |
| doc00.02 | `02-maintenance.md` | Doc philosophy — declarative intent, banned patterns, when to grow detail | docs, maintenance, philosophy | — | — | — |
| doc00.03 | `03-conventions.md` | Cross-reference syntax, frontmatter schema, file naming, writing style | docs, conventions | — | — | — |
| doc00.04 | `04-ai-retrieval.md` | How AI agents navigate this workspace — hierarchical retrieval, MANIFEST usage, token budgets | docs, ai, retrieval | — | — | — |

## 01-product — Product

| Ref | File | Summary | Tags | Deps | Refs | Attachments |
|-----|------|---------|------|------|------|-------------|

| doc01.00 | `00-index.md` |  |  | — | — | — |
| doc01.01 | `01-exploratory.md` | Product vision — emergent agent-based simulation of ancient Mesopotamian debt and ecological cycles, the historical arc it dramatizes, and architecture/visualization directions | product, simulation, abm, history, vision | — | doc01.02.01.00, doc01.03, doc03.01.00, doc03.01.01 | — |
| doc01.03 | `03-unfolding-and-co-evolution.md` | The artifact's two generative processes — a backtrack-free unfolding that builds the world, and a dialectical loop where actors and environment co-determine each other until quantitative accumulation crosses into qualitative change — and the seam between them | product, generation, worldgen, dialectics, emergence, loop-vs-accumulator, philosophy | doc01.01, doc01.02.01.00, doc03.01.06, doc02.01 | doc01.04, doc03.02.00, doc03.02.01 | — |
| doc01.04 | `04-unfolding-world-generation.md` | The method by which the world is generated — a differentiating process that starts from undifferentiated unity and carves it into nested centers, where each step reads the wholeness as it currently stands rather than a field computed once, and the read-set is what separates a world that breathes from one that clumps | worldgen, generation, methodology, unfolding, differentiation, alexander | doc01.03, doc03.01.06, doc02.02 | doc02.02 | — |

### Research Sessions

| Ref | File | Summary | Tags | Deps | Refs | Attachments |
|-----|------|---------|------|------|------|-------------|

| doc01.02.00 | `02-research/00-index.md` | Session-scoped research that informs the product but is not tied to a buildable milestone — each session is a subgroup, each deep-research pass a doc | research, sessions, index | — | — | — |
| doc01.02.01.00 | `02-research/01-mesopotamian-political-economy/00-index.md` | Research session mapping which Mesopotamian economic/ecological variables cycle vs. accumulate until accumulation breaks the cycle — the loop-vs-accumulator thesis behind the simulation | research, mesopotamia, political-economy, debt, ecology, loop-vs-accumulator | doc01.01 | doc01.03 | — |
| doc01.02.01.01 | `02-research/01-mesopotamian-political-economy/01-pass1-debt-and-salinization.md` | Deep-research pass 1: verbatim verified findings on debt-cancellation cycles, the agrarian/commercial debt split, fixed interest, and salinization as a managed-vs-accumulating stress | research, deep-research, debt, jubilee, salinization, silver, loop-vs-accumulator | — | — | — |
| doc01.02.01.02 | `02-research/01-mesopotamian-political-economy/02-pass2-jubilee-politics-and-land-tenure.md` | Deep-research pass 2: verbatim verified findings on the political triggers and legitimacy of debt jubilees, polity as oikos mosaic, the Gelb/Diakonoff land-tenure debate, collapse dynamics, and population as a cycling variable | research, deep-research, jubilee, polity, land-tenure, collapse, population, loop-vs-accumulator | — | — | — |
| doc01.02.01.03 | `02-research/01-mesopotamian-political-economy/03-debt-motives-trade-and-salinization.md` | Why individuals took on debt, how Old Assyrian trade and commoditization worked, and the physical irrigation-to-salt mechanism with the Jacobsen/Adams vs. Powell debate | research, mesopotamia, debt, trade, karum, salinization, loop-vs-accumulator | — | — | — |
| doc01.02.01.04 | `02-research/01-mesopotamian-political-economy/04-household-pastoralists-husbandry-and-resources.md` | Whether the household/oikos is the fundamental political unit, pastoral-settled integration (Mari, dimorphic nomadism, the Amorite question), Ur III institutional herding at scale, and the resource/luxury circuits — all-Sonnet pass with single-vote verify. | research, mesopotamia, household, pastoralism, husbandry, trade, loop-vs-accumulator | — | — | — |
| doc01.02.01.05 | `02-research/01-mesopotamian-political-economy/05-silver-yields-ecology-and-karum-generalizability.md` | The carried-over open threads: silver lifecycle, the technological ratchet/yield evidence, deforestation, carrying capacity, life-cycle/dowry debt, the wheat-to-barley salinity proxy (Powell critique), and whether the Old Assyrian karum/naruqqum model generalizes — Sonnet pipeline with an Opus synthesis merge. | research, mesopotamia, silver, yields, ecology, karum, loop-vs-accumulator | — | — | — |

## 02-design — System

| Ref | File | Summary | Tags | Deps | Refs | Attachments |
|-----|------|---------|------|------|------|-------------|

| doc02.00 | `00-index.md` | The design serves two equally weighted goals — a tuning game the player journeys through, and an aesthetically interesting lifecycle to watch unfold | design, game, aesthetics, lifecycle, goals | — | — | — |
| doc02.01 | `01-reading-the-simulation.md` | The observability practice that makes emergent behavior legible — a ladder from a single agent's decision up to population invariants — and the craft literature behind building simulations as aesthetic experiences | observability, debugging, metrics, tooling, instrumentation, resources, craft | doc05.01, doc04.01 | doc01.03, doc02.03 | — |
| doc02.02 | `02-worldgen-toolkit.md` | The catalogue of CS/algorithm operators that actualize the unfolding method — each one a structure-preserving transformation over the grid state, grouped by the role it plays in the generative sequence, tagged by whether the code calls it or it merely waits in the kit | worldgen, generation, algorithms, operators, toolkit, procgen | doc01.04, doc03.01.06 | doc01.04 | — |
| doc02.03 | `03-the-game.md` | The grazers demo as light play — a three-stage arc from a broken herd, to a working migration the player tunes by hand, to an open score loop where the player cranks scarcity and refines a surviving model for the highest score | design, game, play, score, difficulty, tuning, arc | doc02.01, doc03.01.08 | — | — |

## 03-milestones — Milestones

| Ref | File | Summary | Tags | Deps | Refs | Attachments |
|-----|------|---------|------|------|------|-------------|

| doc03.00 | `00-index.md` | Coherent buildable slices of the simulation, each a present-tense spec of what works on screen at that stage | milestone, index | — | — | — |

### Grazers

| Ref | File | Summary | Tags | Deps | Refs | Attachments |
|-----|------|---------|------|------|------|-------------|

| doc03.01.00 | `01-grazers/00-index.md` | First milestone — a 2D grid ECS where grass grows and elk graze; the simplest slice that proves the simulation loop, and the code structure it establishes | milestone, ecs, grid, simulation | doc01.01 | — | — |
| doc03.01.01 | `01-grazers/01-grazers.md` | First milestone — a 2D grid ECS where grass grows and elk graze; the simplest slice that proves the simulation loop | milestone, ecs, grid, simulation | doc01.01 | doc03.01.02, doc03.01.03, doc03.01.04, doc03.01.05, doc03.01.06, doc03.01.08, doc04.01 | — |
| doc03.01.02 | `01-grazers/02-structure.md` | Crate and module layout for the grazers slice: a binary crate composed of feature plugins | structure, layout, bevy, cargo | doc03.01.01 | doc03.01.07, doc05.01 | — |
| doc03.01.03 | `01-grazers/03-spatial-and-storage-research.md` | Research session during the grazers milestone — spatial representation (grid vs. continuous vs. graph), RimWorld/DF actor routing, ECS storage, network graphs, macro-variable aggregation, and an algorithm inventory for the simulation | research, exploration, spatial, storage, ecs, architecture | doc03.01.01 | doc03.01.04 | — |
| doc03.01.04 | `01-grazers/04-field-and-partition-research.md` | Research session — the Eulerian/Lagrangian field-vs-body faultline under three vocabularies, the field/body decision rule, bodies-over-a-field as one engine, and how milestone-1's grid is the same Eulerian-field-over-a-partition pattern the DESIGN.md graph models use with adjacency swapped from arithmetic to a matrix | research, exploration, spatial, eulerian, lagrangian, partition, graph, ecs | doc03.01.01, doc03.01.03 | doc03.01.05, doc03.01.06, doc03.01.07, doc05.01 | — |
| doc03.01.05 | `01-grazers/05-herd-and-flocking-research.md` | Research session — boids' separation/alignment/cohesion adapted from velocity-steering to per-neighbor move-scoring on a lattice; social foraging (local enhancement) as a two-radius forage sense; a time-growing migration drive; pack affiliation as a partition over bodies; and the two-phase snapshot-then-move ECS pattern that scales to hundreds of bodies | research, exploration, herd, flocking, boids, foraging, migration, ecs, scale | doc03.01.01, doc03.01.04 | doc03.01.09 | — |
| doc03.01.06 | `01-grazers/06-procgen-river-research.md` | Research session — procedural generation as a function from seed and a declarative spec to content; the noise-plus-pathfinding river built generate-and-test over constructive carving; value noise via box-blur smoothing as a cost field; directed least-cost (Dijkstra) carving as a geodesic in an anisotropic metric, with heading, drift, and bendiness; several rivers from one shared cost field spaced by a period; rasterizing a centerline to width with a falloff; and a multi-source BFS turning water distance into a grass carrying-capacity field | research, exploration, procgen, noise, pathfinding, dijkstra, bfs, river, field | doc03.01.01, doc03.01.04 | doc01.03, doc01.04, doc02.02 | — |
| doc03.01.07 | `01-grazers/07-ecs-for-dummies.md` | A ground-up primer on the ECS the grazers slice runs on — entity/component/system/resource, the table/column/archetype storage, structure-of-arrays vs array-of-structs, how lookups and iteration work, and why the layout pays off | ecs, storage, architecture, primer, soa, archetype | doc03.01.02, doc03.01.04 | doc03.01.08 | — |
| doc03.01.08 | `01-grazers/08-elk-decision-model.md` | The conceptual model under an elk's per-tick choice — a flat palette of candidate next-states each scored by one energy-grounded value function, picked by a Boltzmann softmax. Unifies spatial movement drives and the eat/rest decision as gradients of a single potential, and names the terminology the code uses. | ecs, modeling, decision, energy, boids, softmax, dialectics | doc03.01.01, doc03.01.07 | doc02.03, doc03.01.09 | — |
| doc03.01.09 | `01-grazers/09-movement-redesign-research.md` | Why the per-tick drive accumulator buzzes, and the plan-then-steer architecture the movement-ecology and game-AI literatures converge on as its successor — a small per-elk state machine driving steering behaviours (Arrive), with dead-band hysteresis against decision-propagation jitter and an emergent, leaderless follow-chain for the column. | herd, movement, steering, decision, fsm, boids, flocking, buzzing, research | doc03.01.05, doc03.01.08 | — | — |

### Hotspot

| Ref | File | Summary | Tags | Deps | Refs | Attachments |
|-----|------|---------|------|------|------|-------------|

| doc03.02.00 | `02-hotspot/00-index.md` | Second milestone — a rendered ocean-and-sky scene centered on a volcanic hot spot, where islands build up over the source and melt away as they drift off it, carrying their vegetation forward to the next island | milestone, rendering, procgen, lifecycle, co-evolution | doc01.03 | — | — |
| doc03.02.01 | `02-hotspot/01-goal.md` | The hotspot milestone's destination — a rendered ocean-and-sky scene where islands build up over a fixed volcanic source and melt away as they drift off it, with vegetation that co-evolves with the land and jumps from a dying island to a young one | milestone, rendering, procgen, lifecycle, co-evolution | doc01.03 | — | — |

### Driftscape

| Ref | File | Summary | Tags | Deps | Refs | Attachments |
|-----|------|---------|------|------|------|-------------|

| doc03.03.00 | `03-driftscape/00-index.md` | A suite of terminal screensavers, each a drift field past a viewport — the simulation's flow-past-a-frame rendered as truecolor terminal art | milestone, tui, screensaver, drift, parallax | — | — | — |
| doc03.03.01 | `03-driftscape/01-goal.md` | The first Driftscape screensaver — the terminal is a spaceship flying forward through space, planets emerging small at the vanishing point, growing as they approach, and whipping off the sides as they pass while the starfield streaks outward | milestone, tui, perspective, procgen, rendering | — | — | — |

## 04-modeling — Mathematical Modeling

| Ref | File | Summary | Tags | Deps | Refs | Attachments |
|-----|------|---------|------|------|------|-------------|

| doc04.00 | `00-index.md` | The model's formal layer — the equations behind the simulation's forces and metrics, and the discipline for choosing parameter values that hold their meaning | modeling, metrics, parameters | — | — | — |
| doc04.01 | `01-balance-metrics.md` | The equations that define the demo's force decomposition, the migration residual, and the survival/journey/share objective the balancing sweep optimizes | metrics, balance, movement, migration, verification, spec | doc03.01.01, doc05.01 | doc02.01, doc04.02, doc05.01 | — |
| doc04.02 | `02-parameter-discipline.md` | How the model's tunables are chosen so defaults sit in a stable regime — dimensionless ratios over magnitudes, off the bifurcation edge, separated timescales, and sensitivity analysis to find the fragile knobs | parameters, nondimensionalization, sensitivity-analysis, robustness, modeling, methodology | doc04.01 | — | — |

## 05-engineering — Code Quality & Maintenance

| Ref | File | Summary | Tags | Deps | Refs | Attachments |
|-----|------|---------|------|------|------|-------------|

| doc05.00 | `00-index.md` | How the code is shaped and kept maintainable — module boundaries, pure tested functions, feature plugins, and tunables as params | code, patterns, maintenance | — | — | — |
| doc05.01 | `01-coding-patterns.md` | How code is shaped in this project — small modules, pure metrics, feature plugins, the shared field as integration seam, tunables as params, and macro thresholds derived from their determinants | patterns, code, architecture, modules, testing | doc03.01.02, doc03.01.04, doc04.01 | doc02.01, doc04.01 | — |

## Tag Index

Quick lookup for file-path→doc mapping:

| Tag | Relevant Docs |
|-----|---------------|
| `abm` | doc01.01 |
| `aesthetics` | doc02.00 |
| `ai` | doc00.04 |
| `alexander` | doc01.04 |
| `algorithms` | doc02.02 |
| `arc` | doc02.03 |
| `archetype` | doc03.01.07 |
| `architecture` | doc03.01.03, doc03.01.07, doc05.01 |
| `balance` | doc04.01 |
| `bevy` | doc03.01.02 |
| `bfs` | doc03.01.06 |
| `boids` | doc03.01.05, doc03.01.08, doc03.01.09 |
| `buzzing` | doc03.01.09 |
| `cargo` | doc03.01.02 |
| `co-evolution` | doc03.02.00, doc03.02.01 |
| `code` | doc05.00, doc05.01 |
| `collapse` | doc01.02.01.02 |
| `conventions` | doc00.03 |
| `craft` | doc02.01 |
| `debt` | doc01.02.01.00, doc01.02.01.01, doc01.02.01.03 |
| `debugging` | doc02.01 |
| `decision` | doc03.01.08, doc03.01.09 |
| `deep-research` | doc01.02.01.01, doc01.02.01.02 |
| `design` | doc02.00, doc02.03 |
| `dialectics` | doc01.03, doc03.01.08 |
| `differentiation` | doc01.04 |
| `difficulty` | doc02.03 |
| `dijkstra` | doc03.01.06 |
| `docs` | doc00.01, doc00.02, doc00.03, doc00.04 |
| `drift` | doc03.03.00 |
| `ecology` | doc01.02.01.00, doc01.02.01.05 |
| `ecs` | doc03.01.00, doc03.01.01, doc03.01.03, doc03.01.04, doc03.01.05, doc03.01.07, doc03.01.08 |
| `emergence` | doc01.03 |
| `energy` | doc03.01.08 |
| `eulerian` | doc03.01.04 |
| `exploration` | doc03.01.03, doc03.01.04, doc03.01.05, doc03.01.06 |
| `field` | doc03.01.06 |
| `flocking` | doc03.01.05, doc03.01.09 |
| `foraging` | doc03.01.05 |
| `fsm` | doc03.01.09 |
| `game` | doc02.00, doc02.03 |
| `generation` | doc01.03, doc01.04, doc02.02 |
| `goals` | doc02.00 |
| `graph` | doc03.01.04 |
| `grid` | doc03.01.00, doc03.01.01 |
| `herd` | doc03.01.05, doc03.01.09 |
| `history` | doc01.01 |
| `household` | doc01.02.01.04 |
| `husbandry` | doc01.02.01.04 |
| `index` | doc00.00, doc01.02.00, doc03.00 |
| `instrumentation` | doc02.01 |
| `jubilee` | doc01.02.01.01, doc01.02.01.02 |
| `karum` | doc01.02.01.03, doc01.02.01.05 |
| `lagrangian` | doc03.01.04 |
| `land-tenure` | doc01.02.01.02 |
| `layout` | doc03.01.02 |
| `lifecycle` | doc02.00, doc03.02.00, doc03.02.01 |
| `loop-vs-accumulator` | doc01.02.01.00, doc01.02.01.01, doc01.02.01.02, doc01.02.01.03, doc01.02.01.04, doc01.02.01.05, doc01.03 |
| `maintenance` | doc00.02, doc05.00 |
| `mesopotamia` | doc01.02.01.00, doc01.02.01.03, doc01.02.01.04, doc01.02.01.05 |
| `meta` | doc00.00, doc00.01 |
| `methodology` | doc01.04, doc04.02 |
| `metrics` | doc02.01, doc04.00, doc04.01 |
| `migration` | doc03.01.05, doc04.01 |
| `milestone` | doc03.00, doc03.01.00, doc03.01.01, doc03.02.00, doc03.02.01, doc03.03.00, doc03.03.01 |
| `modeling` | doc03.01.08, doc04.00, doc04.02 |
| `modules` | doc05.01 |
| `movement` | doc03.01.09, doc04.01 |
| `noise` | doc03.01.06 |
| `nondimensionalization` | doc04.02 |
| `observability` | doc02.01 |
| `operators` | doc02.02 |
| `parallax` | doc03.03.00 |
| `parameters` | doc04.00, doc04.02 |
| `partition` | doc03.01.04 |
| `pastoralism` | doc01.02.01.04 |
| `pathfinding` | doc03.01.06 |
| `patterns` | doc05.00, doc05.01 |
| `perspective` | doc03.03.01 |
| `philosophy` | doc00.02, doc01.03 |
| `play` | doc02.03 |
| `political-economy` | doc01.02.01.00 |
| `polity` | doc01.02.01.02 |
| `population` | doc01.02.01.02 |
| `primer` | doc03.01.07 |
| `procgen` | doc02.02, doc03.01.06, doc03.02.00, doc03.02.01, doc03.03.01 |
| `product` | doc01.01, doc01.03 |
| `rendering` | doc03.02.00, doc03.02.01, doc03.03.01 |
| `research` | doc01.02.00, doc01.02.01.00, doc01.02.01.01, doc01.02.01.02, doc01.02.01.03, doc01.02.01.04, doc01.02.01.05, doc03.01.03, doc03.01.04, doc03.01.05, doc03.01.06, doc03.01.09 |
| `resources` | doc02.01 |
| `retrieval` | doc00.04 |
| `river` | doc03.01.06 |
| `robustness` | doc04.02 |
| `salinization` | doc01.02.01.01, doc01.02.01.03 |
| `scale` | doc03.01.05 |
| `score` | doc02.03 |
| `screensaver` | doc03.03.00 |
| `sensitivity-analysis` | doc04.02 |
| `sessions` | doc01.02.00 |
| `silver` | doc01.02.01.01, doc01.02.01.05 |
| `simulation` | doc01.01, doc03.01.00, doc03.01.01 |
| `soa` | doc03.01.07 |
| `softmax` | doc03.01.08 |
| `spatial` | doc03.01.03, doc03.01.04 |
| `spec` | doc04.01 |
| `steering` | doc03.01.09 |
| `storage` | doc03.01.03, doc03.01.07 |
| `structure` | doc03.01.02 |
| `testing` | doc05.01 |
| `theory` | doc00.01 |
| `tooling` | doc02.01 |
| `toolkit` | doc02.02 |
| `trade` | doc01.02.01.03, doc01.02.01.04 |
| `tui` | doc03.03.00, doc03.03.01 |
| `tuning` | doc02.03 |
| `unfolding` | doc01.04 |
| `verification` | doc04.01 |
| `vision` | doc01.01 |
| `worldgen` | doc01.03, doc01.04, doc02.02 |
| `yields` | doc01.02.01.05 |
