# .rhidoc/ Manifest

Machine-readable index for AI navigation. Read this file first, then open only the docs relevant to your query.

**Retrieval strategy:** See doc00.04 for AI retrieval patterns.

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
| doc01.01 | `01-exploratory.md` | Product vision — emergent agent-based simulation of ancient Mesopotamian debt and ecological cycles, the historical arc it dramatizes, and architecture/visualization directions | product, simulation, abm, history, vision | — | doc01.02.01.00, doc03.01.00, doc03.01.01 | — |

### Research Sessions

| Ref | File | Summary | Tags | Deps | Refs | Attachments |
|-----|------|---------|------|------|------|-------------|

| doc01.02.00 | `02-research/00-index.md` | Session-scoped research that informs the product but is not tied to a buildable milestone — each session is a subgroup, each deep-research pass a doc | research, sessions, index | — | — | — |
| doc01.02.01.00 | `02-research/01-mesopotamian-political-economy/00-index.md` | Research session mapping which Mesopotamian economic/ecological variables cycle vs. accumulate until accumulation breaks the cycle — the loop-vs-accumulator thesis behind the simulation | research, mesopotamia, political-economy, debt, ecology, loop-vs-accumulator | doc01.01 | — | — |
| doc01.02.01.01 | `02-research/01-mesopotamian-political-economy/01-pass1-debt-and-salinization.md` | Deep-research pass 1: verbatim verified findings on debt-cancellation cycles, the agrarian/commercial debt split, fixed interest, and salinization as a managed-vs-accumulating stress | research, deep-research, debt, jubilee, salinization, silver, loop-vs-accumulator | — | — | — |
| doc01.02.01.02 | `02-research/01-mesopotamian-political-economy/02-pass2-jubilee-politics-and-land-tenure.md` | Deep-research pass 2: verbatim verified findings on the political triggers and legitimacy of debt jubilees, polity as oikos mosaic, the Gelb/Diakonoff land-tenure debate, collapse dynamics, and population as a cycling variable | research, deep-research, jubilee, polity, land-tenure, collapse, population, loop-vs-accumulator | — | — | — |
| doc01.02.01.03 | `02-research/01-mesopotamian-political-economy/03-debt-motives-trade-and-salinization.md` | Why individuals took on debt, how Old Assyrian trade and commoditization worked, and the physical irrigation-to-salt mechanism with the Jacobsen/Adams vs. Powell debate | research, mesopotamia, debt, trade, karum, salinization, loop-vs-accumulator | — | — | — |
| doc01.02.01.04 | `02-research/01-mesopotamian-political-economy/04-household-pastoralists-husbandry-and-resources.md` | Whether the household/oikos is the fundamental political unit, pastoral-settled integration (Mari, dimorphic nomadism, the Amorite question), Ur III institutional herding at scale, and the resource/luxury circuits — all-Sonnet pass with single-vote verify. | research, mesopotamia, household, pastoralism, husbandry, trade, loop-vs-accumulator | — | — | — |
| doc01.02.01.05 | `02-research/01-mesopotamian-political-economy/05-silver-yields-ecology-and-karum-generalizability.md` | The carried-over open threads: silver lifecycle, the technological ratchet/yield evidence, deforestation, carrying capacity, life-cycle/dowry debt, the wheat-to-barley salinity proxy (Powell critique), and whether the Old Assyrian karum/naruqqum model generalizes — Sonnet pipeline with an Opus synthesis merge. | research, mesopotamia, silver, yields, ecology, karum, loop-vs-accumulator | — | — | — |

## 02-design — System

| Ref | File | Summary | Tags | Deps | Refs | Attachments |
|-----|------|---------|------|------|------|-------------|

| doc02.00 | `00-index.md` |  |  | — | — | — |
| doc02.01 | `01-coding-patterns.md` | How code is shaped in this project — small modules, pure metrics, feature plugins, the shared field as integration seam, tunables as params, and macro thresholds derived from their determinants | patterns, code, architecture, modules, testing | doc03.01.02, doc03.01.04, doc02.02 | doc02.02, doc02.03 | — |
| doc02.02 | `02-balance-metrics.md` | The equations that define the demo's force decomposition, the migration residual, and the survival/journey/share objective the balancing sweep optimizes | metrics, balance, movement, migration, verification, spec | doc03.01.01, doc02.01 | doc02.01, doc02.03 | — |
| doc02.03 | `03-reading-the-simulation.md` | The observability practice that makes emergent behavior legible — a ladder from a single agent's decision up to population invariants — and the craft literature behind building simulations as aesthetic experiences | observability, debugging, metrics, tooling, instrumentation, resources, craft | doc02.01, doc02.02 | — | — |

## 03-milestones — Milestones

| Ref | File | Summary | Tags | Deps | Refs | Attachments |
|-----|------|---------|------|------|------|-------------|

| doc03.00 | `00-index.md` | Coherent buildable slices of the simulation, each a present-tense spec of what works on screen at that stage | milestone, index | — | — | — |

### Grazers

| Ref | File | Summary | Tags | Deps | Refs | Attachments |
|-----|------|---------|------|------|------|-------------|

| doc03.01.00 | `01-grazers/00-index.md` | First milestone — a 2D grid ECS where grass grows and elk graze; the simplest slice that proves the simulation loop, and the code structure it establishes | milestone, ecs, grid, simulation | doc01.01 | — | — |
| doc03.01.01 | `01-grazers/01-grazers.md` | First milestone — a 2D grid ECS where grass grows and elk graze; the simplest slice that proves the simulation loop | milestone, ecs, grid, simulation | doc01.01 | doc02.02, doc03.01.02, doc03.01.03, doc03.01.04, doc03.01.05, doc03.01.06 | — |
| doc03.01.02 | `01-grazers/02-structure.md` | Crate and module layout for the grazers slice: a binary crate composed of feature plugins | structure, layout, bevy, cargo | doc03.01.01 | doc02.01 | — |
| doc03.01.03 | `01-grazers/03-spatial-and-storage-research.md` | Research session during the grazers milestone — spatial representation (grid vs. continuous vs. graph), RimWorld/DF actor routing, ECS storage, network graphs, macro-variable aggregation, and an algorithm inventory for the simulation | research, exploration, spatial, storage, ecs, architecture | doc03.01.01 | doc03.01.04 | — |
| doc03.01.04 | `01-grazers/04-field-and-partition-research.md` | Research session — the Eulerian/Lagrangian field-vs-body faultline under three vocabularies, the field/body decision rule, bodies-over-a-field as one engine, and how milestone-1's grid is the same Eulerian-field-over-a-partition pattern the DESIGN.md graph models use with adjacency swapped from arithmetic to a matrix | research, exploration, spatial, eulerian, lagrangian, partition, graph, ecs | doc03.01.01, doc03.01.03 | doc02.01, doc03.01.05, doc03.01.06 | — |
| doc03.01.05 | `01-grazers/05-herd-and-flocking-research.md` | Research session — boids' separation/alignment/cohesion adapted from velocity-steering to per-neighbor move-scoring on a lattice; social foraging (local enhancement) as a two-radius forage sense; a time-growing migration drive; pack affiliation as a partition over bodies; and the two-phase snapshot-then-move ECS pattern that scales to hundreds of bodies | research, exploration, herd, flocking, boids, foraging, migration, ecs, scale | doc03.01.01, doc03.01.04 | — | — |
| doc03.01.06 | `01-grazers/06-procgen-river-research.md` | Research session — procedural generation as a function from seed and a declarative spec to content; the noise-plus-pathfinding river built generate-and-test over constructive carving; value noise via box-blur smoothing as a cost field; directed least-cost (Dijkstra) carving as a geodesic in an anisotropic metric, with heading, drift, and bendiness; several rivers from one shared cost field spaced by a period; rasterizing a centerline to width with a falloff; and a multi-source BFS turning water distance into a grass carrying-capacity field | research, exploration, procgen, noise, pathfinding, dijkstra, bfs, river, field | doc03.01.01, doc03.01.04 | — | — |

## Tag Index

Quick lookup for file-path→doc mapping:

| Tag | Relevant Docs |
|-----|---------------|
| `abm` | doc01.01 |
| `ai` | doc00.04 |
| `architecture` | doc02.01, doc03.01.03 |
| `balance` | doc02.02 |
| `bevy` | doc03.01.02 |
| `bfs` | doc03.01.06 |
| `boids` | doc03.01.05 |
| `cargo` | doc03.01.02 |
| `code` | doc02.01 |
| `collapse` | doc01.02.01.02 |
| `conventions` | doc00.03 |
| `craft` | doc02.03 |
| `debt` | doc01.02.01.00, doc01.02.01.01, doc01.02.01.03 |
| `debugging` | doc02.03 |
| `deep-research` | doc01.02.01.01, doc01.02.01.02 |
| `dijkstra` | doc03.01.06 |
| `docs` | doc00.01, doc00.02, doc00.03, doc00.04 |
| `ecology` | doc01.02.01.00, doc01.02.01.05 |
| `ecs` | doc03.01.00, doc03.01.01, doc03.01.03, doc03.01.04, doc03.01.05 |
| `eulerian` | doc03.01.04 |
| `exploration` | doc03.01.03, doc03.01.04, doc03.01.05, doc03.01.06 |
| `field` | doc03.01.06 |
| `flocking` | doc03.01.05 |
| `foraging` | doc03.01.05 |
| `graph` | doc03.01.04 |
| `grid` | doc03.01.00, doc03.01.01 |
| `herd` | doc03.01.05 |
| `history` | doc01.01 |
| `household` | doc01.02.01.04 |
| `husbandry` | doc01.02.01.04 |
| `index` | doc00.00, doc01.02.00, doc03.00 |
| `instrumentation` | doc02.03 |
| `jubilee` | doc01.02.01.01, doc01.02.01.02 |
| `karum` | doc01.02.01.03, doc01.02.01.05 |
| `lagrangian` | doc03.01.04 |
| `land-tenure` | doc01.02.01.02 |
| `layout` | doc03.01.02 |
| `loop-vs-accumulator` | doc01.02.01.00, doc01.02.01.01, doc01.02.01.02, doc01.02.01.03, doc01.02.01.04, doc01.02.01.05 |
| `maintenance` | doc00.02 |
| `mesopotamia` | doc01.02.01.00, doc01.02.01.03, doc01.02.01.04, doc01.02.01.05 |
| `meta` | doc00.00, doc00.01 |
| `metrics` | doc02.02, doc02.03 |
| `migration` | doc02.02, doc03.01.05 |
| `milestone` | doc03.00, doc03.01.00, doc03.01.01 |
| `modules` | doc02.01 |
| `movement` | doc02.02 |
| `noise` | doc03.01.06 |
| `observability` | doc02.03 |
| `partition` | doc03.01.04 |
| `pastoralism` | doc01.02.01.04 |
| `pathfinding` | doc03.01.06 |
| `patterns` | doc02.01 |
| `philosophy` | doc00.02 |
| `political-economy` | doc01.02.01.00 |
| `polity` | doc01.02.01.02 |
| `population` | doc01.02.01.02 |
| `procgen` | doc03.01.06 |
| `product` | doc01.01 |
| `research` | doc01.02.00, doc01.02.01.00, doc01.02.01.01, doc01.02.01.02, doc01.02.01.03, doc01.02.01.04, doc01.02.01.05, doc03.01.03, doc03.01.04, doc03.01.05, doc03.01.06 |
| `resources` | doc02.03 |
| `retrieval` | doc00.04 |
| `river` | doc03.01.06 |
| `salinization` | doc01.02.01.01, doc01.02.01.03 |
| `scale` | doc03.01.05 |
| `sessions` | doc01.02.00 |
| `silver` | doc01.02.01.01, doc01.02.01.05 |
| `simulation` | doc01.01, doc03.01.00, doc03.01.01 |
| `spatial` | doc03.01.03, doc03.01.04 |
| `spec` | doc02.02 |
| `storage` | doc03.01.03 |
| `structure` | doc03.01.02 |
| `testing` | doc02.01 |
| `theory` | doc00.01 |
| `tooling` | doc02.03 |
| `trade` | doc01.02.01.03, doc01.02.01.04 |
| `verification` | doc02.02 |
| `vision` | doc01.01 |
| `yields` | doc01.02.01.05 |
