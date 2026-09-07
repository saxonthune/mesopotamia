---
title: Structure
summary: Crate and module layout for the grazers slice: a binary crate composed of feature plugins
tags: [structure, layout, bevy, cargo]
deps: [doc03.01.01]
---

# Structure

The grazers slice is a single binary crate. Its code is organized the way Bevy code is idiomatically organized: by **feature plugin**. Each feature owns its components, resources, and systems, and exposes a `Plugin` that registers them. `main.rs` is a thin composition root that adds Bevy's `DefaultPlugins` and the feature plugins, then runs the app.

## Crate Layout

```
mesopotamia/
├── Cargo.toml
├── assets/              # textures and sprites; Bevy loads paths relative to here
└── src/
    ├── main.rs          # App composition: DefaultPlugins + feature plugins
    ├── grid.rs          # GridPlugin: Grid resource, Cell state, growth system
    ├── elk/
    │   ├── mod.rs       # ElkSimPlugin + pub use re-exports (PACK_COUNT, Elk, …)
    │   ├── components.rs  # Elk, Cohort, Herds, Packs, Spawner, ElkParams
    │   ├── spawn.rs     # spawn_pack, spawn_waves, cull, tally_herds
    │   ├── movement.rs  # herd_move, norm, step_water_penalty, cross_desire
    │   ├── metabolism.rs  # graze, digest, metabolize, migrate_pressure
    │   └── color.rs     # elk_color (pub(crate))
    └── render.rs        # RenderPlugin: camera, grid/elk → sprites
```

Modules use the `foo.rs` form (a file beside `src/`) until they hold enough to split. `elk` uses the `foo/mod.rs` + `foo/` directory pair: `elk/mod.rs` is the module root (plugin declaration and public re-exports); the submodules split along simulation seams.

## Feature Plugins

A feature is a module that exposes a `Plugin`. The plugin's `build` registers everything that feature needs — resources, systems, and their schedule:

```rust
pub struct GridPlugin;

impl Plugin for GridPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Grid>()
            .add_systems(FixedUpdate, growth);
    }
}
```

The composition root stays small:

```rust
fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins((GridPlugin, ElkSimPlugin, RenderPlugin))
        .run();
}
```

## Sim / Render Split

`elk/mod.rs` exposes `ElkSimPlugin`, which owns elk simulation: the `Elk` component, its resources (`Packs`, `Spawner`, `ElkParams`, `Herds`), and all `FixedUpdate` systems (`herd_move`, `graze`, `digest`, `metabolize`, `migrate_pressure`, `spawn_waves`, `cull`) plus `tally_herds` on `Update`. It carries no rendering dependency and loads cleanly in a headless context. External paths (`mesopotamia::elk::Elk`, `elk_color`, etc.) are preserved by `pub use` re-exports in `elk/mod.rs`.

`render.rs` exposes `RenderPlugin`, which owns everything that reflects world state into sprites. This includes the grid tile and poop/browse layers, the camera, and the two elk-sync systems: `sync_elk_transform` (interpolates each elk sprite between its previous and current cell at render rate) and `sync_elk_color` (tints each sprite from the elk's `grazing` flag). `RenderPlugin` imports `elk_color` from `elk` for the tint calculation.

`tests/macro_sim.rs` uses `ElkSimPlugin` directly, with no render dependency, to run the population, grass, and migration invariants headlessly.

## Sim State

A `Sim` state (`Generating` → `Running`) lives in `src/sim.rs`. World generation runs in `OnEnter(Sim::Generating)` and transitions to `Running` once the grid is built. Grid and elk `FixedUpdate` systems are gated on `Running`. A regenerate draws a fresh `WorldSeed` and transitions back to `Generating`, which clears the grid (worldgen) and despawns all elk + resets their lifecycle resources (`OnExit(Running)`) — preserving the tuned `ElkParams`, `GrowthRate`, and `Fertility` across regenerates.

## Schedules

Simulation systems (growth, movement, grazing) run on `FixedUpdate` so the world steps at a fixed rate independent of framerate, in the order the tick defines (doc03.01.01). These systems are gated on `Sim::Running` so they are silent during world generation. Rendering — camera setup and reflecting grid and elk state into sprites — runs on `Startup` and `Update`.

## Cargo

`Cargo.toml` depends on `bevy`. Bevy compiles slowly in a plain debug build, so the dev profile raises the optimization level of dependencies:

```toml
[profile.dev.package."*"]
opt-level = 3
```

This keeps the project's own code fast to recompile while the heavy engine crates stay optimized.
