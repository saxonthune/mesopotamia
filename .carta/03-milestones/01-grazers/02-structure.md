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
    ├── elk.rs           # ElkPlugin: Elk component, movement + grazing systems
    └── render.rs        # RenderPlugin: camera, grid/elk → sprites
```

Modules use the `foo.rs` form (a file beside `src/`), not `foo/mod.rs`. A module grows into a `foo.rs` + `foo/` directory pair only when it holds enough to split.

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
        .add_plugins((GridPlugin, ElkPlugin, RenderPlugin))
        .run();
}
```

## Schedules

Simulation systems (growth, movement, grazing) run on `FixedUpdate` so the world steps at a fixed rate independent of framerate, in the order the tick defines (doc03.01.01). Rendering — camera setup and reflecting grid and elk state into sprites — runs on `Startup` and `Update`.

## Cargo

`Cargo.toml` depends on `bevy`. Bevy compiles slowly in a plain debug build, so the dev profile raises the optimization level of dependencies:

```toml
[profile.dev.package."*"]
opt-level = 3
```

This keeps the project's own code fast to recompile while the heavy engine crates stay optimized.
