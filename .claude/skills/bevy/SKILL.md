---
name: bevy
description: Reference for the Bevy 0.17 game engine in Rust — ECS (components, resources, systems, queries, commands), 2D rendering (Camera2d, Sprite, Mesh2d), schedules, time, input, and messages. Use when writing, reading, or reviewing Bevy 0.17 code.
---

# Bevy 0.17

Bevy is a data-oriented Rust game engine built on an Entity-Component-System (ECS) core. This skill is a condensed, version-accurate reference for **Bevy 0.17** (Rust edition 2024). It is grounded in the 0.17 example source, not older tutorials — Bevy changes APIs between releases, so prefer what is written here over memory of earlier versions.

Everything comes from one prelude:

```rust
use bevy::prelude::*;
```

## The Mental Model

- **Entity** — an opaque id. Holds no data itself.
- **Component** — a plain Rust type attached to an entity. `#[derive(Component)]`.
- **Resource** — a single global value, not tied to any entity. `#[derive(Resource)]`.
- **System** — a plain function whose parameters declare what it reads and writes. Bevy runs systems in parallel when their data access does not conflict.
- **Schedule** — a named phase that holds systems: `Startup` (once), `Update` (every frame), `FixedUpdate` (fixed timestep).

Logic lives in systems; state lives in components and resources. You never call a system yourself — you register it on a schedule and Bevy drives it.

## The Smallest App

```rust
use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)      // window, render, input, time, ...
        .add_systems(Startup, setup)      // runs once
        .add_systems(Update, tick)        // runs every frame
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
}

fn tick(time: Res<Time>) {
    // per-frame logic
}
```

`App` is a builder: `.add_plugins(...)`, `.add_systems(schedule, systems)`, `.insert_resource(...)`, `.init_resource::<T>()`, `.run()`.

## Defining Data

```rust
#[derive(Component)]
struct Velocity(Vec2);

#[derive(Component)]
enum State { Idle, Moving }      // enums work as components too

#[derive(Resource, Default)]
struct Score { value: u32 }
```

Add resources two ways:
- `app.init_resource::<Score>()` — requires `Default` (or `FromWorld`).
- `app.insert_resource(Score { value: 10 })` — provide the value yourself.

## Systems and Their Parameters

A system is a function; each parameter is a request to the ECS:

| Parameter | Gives you |
|---|---|
| `Commands` | queue spawns, despawns, resource inserts (applied after the system) |
| `Res<T>` / `ResMut<T>` | read / read-write a resource |
| `Query<...>` | iterate entities matching a component set |
| `Local<T>` | per-system private state, persists across runs |
| `Res<Time>` | frame timing |
| `Res<ButtonInput<KeyCode>>` | keyboard state |
| `MessageWriter<T>` / `MessageReader<T>` | send / receive messages |

```rust
fn movement(time: Res<Time>, mut query: Query<(&Velocity, &mut Transform)>) {
    for (vel, mut transform) in &mut query {
        transform.translation += vel.0.extend(0.0) * time.delta_secs();
    }
}
```

Iterate `&query` for read-only, `&mut query` when any component is `&mut`. See `references/ecs-core.md` for query filters (`With`, `Without`), `Single`, `get`, commands, and ordering.

## Schedules and Time

- `Startup` — once, before `Update`. Build the initial world here.
- `Update` — once per frame; framerate-dependent. Use `time.delta_secs()` to stay frame-rate independent.
- `FixedUpdate` — fixed timestep, ideal for deterministic simulation steps:

```rust
app.insert_resource(Time::<Fixed>::from_seconds(0.5))   // twice a second
   .add_systems(FixedUpdate, step);
```

`Res<Time>`: `time.delta_secs()` (f32 since last run), `time.elapsed_secs()`, `time.delta()` (`Duration`). For timers: `Timer::from_seconds(1.0, TimerMode::Repeating)`, then `timer.tick(time.delta()).is_finished()`.

## Ordering

Systems in a schedule run in parallel by default. Force order when it matters:

```rust
app.add_systems(Update, (growth, movement, grazing).chain());   // strict order
// or .before(other) / .after(other), or named SystemSets — see references/ecs-core.md
```

## 2D Rendering (essentials)

```rust
fn setup(mut commands: Commands, assets: Res<AssetServer>) {
    commands.spawn(Camera2d);

    // image sprite
    commands.spawn(Sprite::from_image(assets.load("icon.png")));

    // solid-color rectangle sprite (size in world units)
    commands.spawn((
        Sprite::from_color(Color::srgb(0.2, 0.7, 0.3), Vec2::new(32.0, 32.0)),
        Transform::from_xyz(100.0, 0.0, 0.0),
    ));
}
```

Spawning a `Sprite` automatically pulls in a `Transform` (required components). Full struct form: `Sprite { image, custom_size: Some(Vec2), color, ..default() }`. For meshes/shapes use `Mesh2d` + `MeshMaterial2d`. See `references/render-2d.md`.

## The One Big Gotcha: No Bundles, and Events Are Messages

Bevy 0.17 **removed `*Bundle` types** (`Camera2dBundle`, `SpriteBundle`, …). Spawn the component directly; "required components" pull in the rest. And **events were renamed to messages** (`EventWriter` → `MessageWriter`, `add_event` → `add_message`, `.send()` → `.write()`). Older tutorials predate both changes. Always check `references/0_17-api-changes.md` before trusting code found online.

## References

Load these for depth — they hold verified 0.17 snippets:

- **`references/ecs-core.md`** — components, resources, queries (filters, `Single`, `get`), commands, system ordering, sets, `Local`, messages.
- **`references/render-2d.md`** — `Camera2d`, `Sprite` (image/color/atlas), `Transform`, `Color`, `Mesh2d`/`MeshMaterial2d`, coordinate conventions.
- **`references/0_17-api-changes.md`** — what renamed/moved vs older Bevy: bundles, events→messages, `delta_secs`, and other traps when adapting old code.
