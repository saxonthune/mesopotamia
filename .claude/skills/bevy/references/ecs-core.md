# ECS Core (Bevy 0.17)

Verified against the 0.17 example source. `use bevy::prelude::*;` provides every type below.

## Components

A component is any Rust type with `#[derive(Component)]`:

```rust
#[derive(Component)]
struct Position { x: i32, y: i32 }

#[derive(Component)]
struct Health(u32);              // tuple struct

#[derive(Component)]
enum Direction { Left, Right }   // enums are fine
```

Attach components by spawning them on an entity (see Commands). One entity may carry any combination of component types — but only one instance of each type.

## Resources

A single global value:

```rust
#[derive(Resource, Default)]
struct GameState { round: usize }

#[derive(Resource)]
struct Rules { max_rounds: usize }
```

Register on the app:

```rust
app.init_resource::<GameState>()           // needs Default or FromWorld
   .insert_resource(Rules { max_rounds: 10 });
```

Read/write inside a system with `Res<T>` (shared) / `ResMut<T>` (exclusive):

```rust
fn new_round(rules: Res<Rules>, mut state: ResMut<GameState>) {
    state.round += 1;
}
```

## Commands

`Commands` queues structural changes; they apply after the system finishes.

```rust
fn setup(mut commands: Commands) {
    // spawn one entity with several components (a tuple)
    let id = commands.spawn((
        Position { x: 0, y: 0 },
        Health(100),
        Direction::Right,
    )).id();                       // .id() returns the Entity

    commands.spawn_batch(vec![     // many entities at once
        (Position { x: 1, y: 0 }, Health(50)),
        (Position { x: 2, y: 0 }, Health(50)),
    ]);

    commands.entity(id).insert(Velocity(Vec2::ZERO));  // add a component later
    commands.entity(id).despawn();                     // remove the entity

    commands.insert_resource(Rules { max_rounds: 5 });
}
```

There is no bundle type — a "bundle" is just a tuple of components. Spawning a component that declares **required components** auto-inserts those too (e.g. spawning `Sprite` adds `Transform`).

## Queries

A `Query` iterates entities that have the requested components.

```rust
// read-only: iterate &query
fn report(query: Query<(&Position, &Health)>) {
    for (pos, hp) in &query {
        info!("at {},{} hp {}", pos.x, pos.y, hp.0);
    }
}

// mutable: iterate &mut query, take components as &mut
fn heal(mut query: Query<&mut Health>) {
    for mut hp in &mut query {
        hp.0 += 1;                 // Mut<T> derefs to T
    }
}
```

### Filters

Second type parameter filters without fetching data:

```rust
Query<&Transform, With<Player>>        // has Player, but don't fetch it
Query<&mut Transform, Without<Camera>> // exclude entities with Camera
Query<Entity, (With<Enemy>, With<Alive>)>  // combine with a tuple
```

`With`/`Without` are filters; put the data you actually read in the first parameter.

### Single entity

When exactly one entity matches:

```rust
fn camera(query: Single<&Transform, With<Camera2d>>) {
    let t = query.into_inner();
}
```

Or fetch by id from a normal query: `query.get(entity)` → `Result`, `query.get_mut(entity)`.

## System Parameters Cheat-Sheet

| Param | Purpose |
|---|---|
| `Commands` | queue spawn/despawn/insert (deferred) |
| `Res<T>` / `ResMut<T>` | read / read-write a resource |
| `Query<D, F>` | iterate entities; `D` data, `F` filter |
| `Single<D, F>` | exactly-one-match shorthand |
| `Local<T>` | private per-system state across runs (init via `Default`) |
| `Res<Time>` | timing |
| `Res<ButtonInput<KeyCode>>` | keyboard; `.pressed` / `.just_pressed` / `.just_released` |
| `MessageWriter<T>` / `MessageReader<T>` | send / receive messages |

```rust
fn count(mut n: Local<u32>) { *n += 1; }   // Local persists between frames
```

## Ordering Systems

Systems in a schedule run in parallel unless their data access conflicts. To force order:

```rust
// run in listed order, every frame
app.add_systems(Update, (growth, movement, grazing).chain());

// relative ordering
app.add_systems(Update, (a, b.after(a), c.before(a)));
```

### Named system sets

For coarse ordering across many systems:

```rust
#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
enum Phase { Input, Sim, Render }

app.configure_sets(Update, (Phase::Input, Phase::Sim, Phase::Render).chain())
   .add_systems(Update, read_keys.in_set(Phase::Input))
   .add_systems(Update, (step_a, step_b).in_set(Phase::Sim));
```

## Messages (formerly Events)

In 0.17, "events" are **messages**. Define, register, write, read:

```rust
#[derive(Message)]
struct Damage { amount: i32 }

app.add_message::<Damage>();

fn deal(mut writer: MessageWriter<Damage>) {
    writer.write(Damage { amount: 10 });   // .write_default() if it derives Default
}

fn apply(mut reader: MessageReader<Damage>) {
    for dmg in reader.read() {
        info!("took {}", dmg.amount);
    }
}
```

`MessageMutator<T>` lets a middle system mutate messages in place. Order writer-before-reader with `.chain()` to avoid a one-frame delay.

To quit the app, write the built-in message:

```rust
fn quit(mut exit: MessageWriter<AppExit>) {
    exit.write(AppExit::Success);
}
```
