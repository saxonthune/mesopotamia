# 0.17 API Traps (vs. Older Bevy)

Bevy changes APIs between releases, and most tutorials, blog posts, and model memory predate 0.17. These are the differences that bite when adapting older code. All confirmed against the 0.17 example source.

## Bundles Are Gone

Older Bevy spawned `*Bundle` structs. **0.17 has no bundle types.** Spawn the component directly; its "required components" auto-insert the rest.

| Old (≤0.13-ish) | 0.17 |
|---|---|
| `commands.spawn(Camera2dBundle::default())` | `commands.spawn(Camera2d)` |
| `commands.spawn(SpriteBundle { texture, transform, ..default() })` | `commands.spawn((Sprite::from_image(texture), transform))` |
| `commands.spawn(Camera3dBundle { .. })` | `commands.spawn(Camera3d)` |

A "bundle" is now just a tuple of components: `commands.spawn((A, B, C))`. Spawning `Sprite` automatically adds `Transform` because `Sprite` *requires* it.

## Events Are Now Messages

The event system was renamed wholesale.

| Old | 0.17 |
|---|---|
| `#[derive(Event)]` | `#[derive(Message)]` |
| `app.add_event::<T>()` | `app.add_message::<T>()` |
| `EventWriter<T>` | `MessageWriter<T>` |
| `EventReader<T>` | `MessageReader<T>` |
| `writer.send(x)` | `writer.write(x)` |
| `writer.send_default()` | `writer.write_default()` |
| *(new)* | `MessageMutator<T>` — mutate messages mid-pipeline |

`AppExit` is a message: `MessageWriter<AppExit>`, `exit.write(AppExit::Success)`.

## Time Method Names

| Old | 0.17 |
|---|---|
| `time.delta_seconds()` | `time.delta_secs()` |
| `time.elapsed_seconds()` | `time.elapsed_secs()` |
| `timer.finished()` | `timer.is_finished()` |

`time.delta()` still returns a `Duration`. Fixed timestep: `app.insert_resource(Time::<Fixed>::from_seconds(0.5))` and register systems on `FixedUpdate`.

## Query Iteration

Iterate the query reference, not the query value:

```rust
for x in &query { }        // read-only
for mut x in &mut query { } // mutable
```

(Older `query.iter()` / `query.iter_mut()` still exist but `&query` / `&mut query` is the idiom in 0.17 examples.)

## Sprite Construction

No `SpriteBundle`. Build `Sprite` directly:

- `Sprite::from_image(handle)`
- `Sprite::from_color(color, size_vec2)`
- `Sprite { image, custom_size: Some(size), color, ..default() }`

## Color Constructors

Use the explicit color-space constructors: `Color::srgb(r,g,b)`, `Color::srgba(..)`, `Color::hsl(h,s,l)`. Older `Color::rgb(..)` shorthand is replaced by `Color::srgb(..)`.

## Toolchain

Bevy 0.17 targets **Rust edition 2024**. Ensure a recent stable toolchain. `rand` is an external crate (used in examples via `rand::random()`), not part of Bevy.

## When In Doubt

The vendored 0.17 source is the ground truth. Read the matching example under `examples/2d/` or `examples/ecs/` rather than trusting an older snippet. If a symbol does not resolve, it was likely renamed in the table above.
