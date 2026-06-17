# Declarative directional rivers (top→bottom drift, two channels, anisotropic carve)

## Motivation

`src/river.rs` hardcodes its parameters as module-level `const`s and carves a single main
channel **left→right** with meander driven only by noise frequency. Reconcile the code to the
present-tense design in **doc03.01.06** (procgen river research): the watershed is authored by a
small **declarative `RiverSpec`**, rivers flow **top→bottom with a slight rightward drift**, there
are **two** of them spaced apart (so elk perform crossings), and the channel is carved as a
**geodesic in an anisotropic cost metric** (heading/drift expressed as directional cost). Read
doc03.01.06 first — it is the spec; if you find it wrong, fix the doc in place rather than diverge.

Decisions locked with the user (do not revisit):
- **Anisotropic Dijkstra**, not a parametric 1D-noise centerline.
- **World size 256 × 144** (16:9, up from 256 × 64).
- **Single `[0,1]` `bendiness` knob** derives both smoothing passes (wavelength) and the
  directional penalty weight — one intuitive control, not two separate dials in the spec.
- **Minimal purity**: thread a `RiverSpec` through the existing `&mut Grid` functions; keep
  operating on `&Grid`/`&mut Grid`. `generate_river_inner` stays Bevy-free and deterministic.
- **Tributaries are carved AFTER the main rivers and branch OFF them** — each tributary joins a
  main channel at a confluence cell sampled from that main river's centerline. They are not
  independent edge-to-edge carves.

## Do NOT

- **Do NOT** adopt the parametric 1D-noise centerline approach. Keep noise + Dijkstra.
- **Do NOT** extract a `generate() -> Fields` pure function or hoist `step` to `field.rs`. The
  user chose the minimal refactor: thread the spec through the existing `&mut Grid` helpers.
- **Do NOT** make tributaries random independent edge-to-edge carves. A tributary's goal is a
  cell ON a main river's centerline (a confluence); only the main rivers use the heading/drift bias.
- **Do NOT** change render/UI, elk behaviour, migration (`EDGE_COL`), fords-as-barrier logic, or
  `is_ford`'s consumer. The render layer already reads `GRID_HEIGHT`/`GRID_WIDTH` constants, so
  the height bump flows through with no render edit.
- **Do NOT** break determinism: same `(seed, spec)` must yield an identical water field.
- **Do NOT** make `WATER_REACH`/capacity reach scale with grid size (separate open question).
  Only flag in the result notes if the green band looks visibly wrong at 256×144.
- **Do NOT** introduce branching deltas or space-filling drainage networks.
- **Do NOT** turn `RiverSpec` into a UI-tunable Bevy `Resource` — a plain struct with `Default`
  is the scope. (A comment noting it could become a resource later is fine.)

## Plan

### 1. World size — `src/grid.rs`

Change `GRID_HEIGHT` from `64` to `144`; keep `GRID_WIDTH = 256`. Nothing else in `grid.rs`
changes. Confirm `render.rs` needs no edit (it reads the constants).

### 2. Introduce `RiverSpec` — `src/river.rs`

A plain struct with a `Default`, replacing the loose `const`s:

```rust
pub struct RiverSpec {
    pub count: usize,        // main rivers (default 2)
    pub period: usize,       // columns between adjacent entry points on the top edge
    pub heading: Heading,    // primary flow direction (default Down)
    pub drift: f32,          // lateral lean: exit_col = entry_col + drift * height (default ~0.25)
    pub bendiness: f32,      // [0,1]; 0 = nearly straight, 1 = wanders far off heading (default ~0.5)
    pub tributaries: usize,  // shallow feeders branching off the mains (default 2)
    // retained shaping consts that aren't part of the bendiness knob:
    pub radius: isize,       // main-channel raster radius (was RIVER_RADIUS = 3)
    pub core: isize,         // full-depth core radius (was RIVER_CORE = 1)
    pub water_reach: u32,    // capacity BFS reach (was WATER_REACH = 8)
    pub trib_radius: isize, pub trib_depth: f32,
    pub oxbow_count: usize, pub oxbow_radius: isize, pub oxbow_depth: f32,
    pub ford_spacing: usize, pub ford_depth: f32,
}

pub enum Heading { Down }   // only Down is exercised; keep the enum minimal but extensible
```

`Heading` drives (a) which edge the entries sit on (top, row 0) and (b) the penalty axis. Keep
`RIVER_SEED` as a const (or a `seed` field — your call; default `0xBEDA`).

`bendiness` derivation (tune the numeric ranges by eye, keep the shape):
- `passes  = round(lerp(8.0, 2.0, bendiness))` — low bendiness → more passes → broad gentle bends;
  high bendiness → fewer passes → tight wiggles.
- `penalty = lerp(PEN_HIGH, PEN_LOW, bendiness)` as an integer cost added to against-heading steps;
  low bendiness → high penalty → forced straight; high bendiness → low penalty → free to wander.
  Pick `PEN_HIGH`/`PEN_LOW` relative to the `(v*1000)+1` cost scale so the contest is visible.

### 3. Anisotropic carve — `src/river.rs`

Give `carve` an optional directional bias. Signature like
`carve(grid, cost, start, goal, bias: Option<(Heading, u32)>)`. When `bias` is `Some((heading, penalty))`,
the edge weight to neighbour `n` via `(dx, dy)` becomes `cost[n] + dir_penalty(dx, dy, heading, penalty)`:

- For `Heading::Down` (row increases downward; entry row 0, exit row `height-1`):
  - step **down** (`dy = +1`): penalty `0` (with-heading, cheap)
  - step **up** (`dy = -1`): penalty `penalty` (against-heading, dear)
  - step **sideways** (`dx = ±1`): penalty `0` (free meander; drift comes from the goal offset)
- When `bias` is `None`, weight is just `cost[n]` (plain least-cost — used by tributaries).

The lateral **drift** is the goal offset, not a horizontal penalty: keep it in the endpoint math
(step 4). This preserves the "geodesic in a directed metric" framing in doc03.01.06.

### 4. Main rivers from the spec — `src/river.rs` (`generate_river_inner`)

Rewrite `generate_river_inner` to take `&RiverSpec` and return the carved main centerlines for
tests, e.g. `fn generate_river_inner(grid: &mut Grid, spec: &RiverSpec) -> Vec<Vec<usize>>`:

1. Build ONE shared cost field via `cost_field(grid, &mut rng, passes)` (passes derived from
   `bendiness`). All channels carve on this same field so they read as one landscape.
2. Place `count` entry columns spaced `period` apart, centred:
   `span = (count-1)*period; start_col = (width - span)/2; entry_col(i) = start_col + i*period`
   (clamp into `[0, width-1]`). Entry index = `entry_col` (row 0).
3. Exit column = `(entry_col as f32 + drift * height as f32)` rounded, clamped to `[0, width-1]`;
   exit index = `(height-1)*width + exit_col` (bottom edge).
4. `carve(grid, &cost, entry, exit, Some((heading, penalty)))` per river; `rasterize(grid, &cl,
   spec.radius, 1.0)`. Collect centerlines.

### 5. Tributaries branch off the mains — `src/river.rs`

After the mains are carved, add `spec.tributaries` shallow feeders, each joining a main channel:

1. Pick a main centerline at random, then a **confluence** cell from its interior (exclude the
   first/last ~10% so feeders don't graze the entry/exit). This is the tributary's **goal**.
2. Pick a **source** on a side edge (left col 0 or right col `width-1`) at a random row in the
   upper portion of the map (so the feeder descends toward the confluence).
3. `carve(grid, &cost, source, confluence, None)` — plain least-cost on the shared field, no
   heading bias — then `rasterize(grid, &cl, spec.trib_radius, spec.trib_depth)`.

This guarantees each tributary connects to a main river by construction. Keep oxbows and fords as
they are today (oxbows = local cost minima; fords = periodic shallow bands along EACH main
centerline — loop `ford_indices` over every main centerline, not just one). Finally
`compute_water_prox(grid, spec.water_reach)`.

### 6. Plugin wiring — `src/river.rs`

`generate_river` (the `Startup` system) builds a `RiverSpec::default()` and calls
`generate_river_inner(&mut grid, &spec)`, discarding the returned centerlines. `RiverPlugin`
unchanged otherwise. Keep the existing `field::smooth` usage in `cost_field`.

### 7. Tests — `src/river.rs`

- Build grids at a representative tall size using the real constants where practical
  (`Grid::new(GRID_WIDTH, GRID_HEIGHT)` or e.g. `Grid::new(128, 96)`) — **height > implied by the
  old 128×32**, since rivers now run top→bottom.
- `ford_indices_*` tests: unchanged (pure `Vec` function).
- `generation_is_deterministic`: same seed+spec → identical water field. Keep.
- `fords_carry_low_water`, `main_channel_has_deep_water`, `water_levels_span_expected_range`:
  adapt to the new size; keep their intent.
- **New** `rivers_flow_top_to_bottom`: from the returned main centerlines, assert each river's
  entry is in row 0 and exit in row `height-1`, that `exit_col > entry_col` (rightward drift),
  and that `count` distinct main channels exist.
- **New** `tributaries_join_a_main_channel`: assert each tributary centerline's goal cell is a
  carved main-channel cell (water > tributary depth), i.e. the feeder reaches a main river.

### 8. Reconcile doc03.01.06 if needed

If the single-`bendiness`-knob design conflicts with any wording (the doc currently frames
wavelength and bendiness as "two independent dials"), adjust that one sentence in place so it
reads that a single `bendiness` control drives both the smoothing passes (wavelength) and the
directional penalty. Also ensure the tributary text matches "branches off a main channel at a
confluence." Present-tense; rewrite in place; run `rhidoc regenerate` if frontmatter changes
(the CLI is `rhidoc`, not `carta`).

## Files to Modify

- `src/grid.rs` — `GRID_HEIGHT` 64 → 144.
- `src/river.rs` — `RiverSpec` + `Heading`; anisotropic `carve` bias; top→bottom drift endpoints;
  `count` mains spaced by `period`; tributaries branching off mains to a confluence; fords over
  every main centerline; thread `spec` everywhere; adapt + add tests.
- `.carta/03-milestones/01-grazers/06-procgen-river-research.md` — only if wording needs the
  single-knob / confluence reconcile (then `rhidoc regenerate`).

## Verification

```bash
cargo test
cargo build --bin mesopotamia
cargo build --bin demo1
cargo clippy --all-targets -- -D warnings
```

Visual (manual, note in result): two rivers descend top→bottom drifting right on the 256×144 map,
with shallow tributaries feeding into them and periodic fords across each main channel.

## Out of Scope

- Parametric 1D-noise centerline; branching deltas / drainage networks.
- Capacity-reach-vs-grid-scale tuning (flag only).
- Elk fording / water gating body movement (`is_ford` consumer is a later phase).
- Render/UI changes beyond what the constants already propagate.
- `RiverSpec` as a UI-tunable Bevy resource.

## Notes

- `src/field.rs` is pure and stays untouched; reuse `field::smooth` in `cost_field` as today.
- Orientation: index = `row*width + col`, row 0 is the top, `dy = +1` moves down — so
  `Heading::Down` favours `dy = +1` and penalises `dy = -1`.
- `demo1.rs` shares `river.rs` via `#[path]`, so it picks up changes automatically — just keep it
  compiling (the `cargo build --bin demo1` gate).
- Determinism: tributary/oxbow/confluence random draws all come from the single seeded `StdRng`
  threaded through generation — do not introduce a second RNG or touch the clock.
- The two mains share ONE cost field (carve both on the same smoothed noise) per doc03.01.06.
