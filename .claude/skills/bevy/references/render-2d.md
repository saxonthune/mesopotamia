# 2D Rendering (Bevy 0.17)

Verified against the 0.17 `examples/2d` source. `use bevy::prelude::*;`.

## Camera

A 2D scene needs a `Camera2d`. Spawn it once, usually in `Startup`:

```rust
commands.spawn(Camera2d);
```

`Camera2d` is a component (not a bundle). The world origin `(0,0)` sits at screen center; +x is right, +y is up. Z controls draw order for sprites (higher z draws on top).

## Sprites

`Sprite` is the 2D image/quad component. Three common forms:

```rust
// from an image asset
commands.spawn(Sprite::from_image(assets.load("player.png")));

// a solid-color rectangle, sized in world units
commands.spawn(Sprite::from_color(Color::srgb(0.2, 0.7, 0.3), Vec2::new(32.0, 32.0)));

// full struct for more control
commands.spawn(Sprite {
    image: assets.load("tile.png"),
    custom_size: Some(Vec2::new(64.0, 64.0)),   // override native size
    color: Color::WHITE,                         // tint
    ..default()
});
```

Spawning a `Sprite` auto-adds a `Transform` (required component). To place or move it, include a `Transform`:

```rust
commands.spawn((
    Sprite::from_color(Color::srgb(0.8, 0.1, 0.1), Vec2::splat(16.0)),
    Transform::from_xyz(100.0, -40.0, 0.0),
));
```

Loading images needs `Res<AssetServer>`; asset paths are relative to the `assets/` directory.

## Transform

`Transform` holds translation, rotation, scale.

```rust
Transform::from_xyz(x, y, z)
Transform::from_translation(Vec3::new(x, y, z))

// mutate in a system
transform.translation.x += 150.0 * time.delta_secs();
transform.translation += velocity.extend(0.0) * time.delta_secs();   // Vec2 -> Vec3
```

`Vec2::new(x, y)`, `Vec2::splat(n)`, `Vec2::ZERO`; `vec2.extend(z)` → `Vec3`.

## Color

```rust
Color::srgb(r, g, b)        // 0.0..=1.0 sRGB
Color::srgba(r, g, b, a)
Color::hsl(hue, sat, light) // hue in degrees 0..360
Color::WHITE, Color::BLACK
```

## Meshes and Shapes

For vector shapes rather than image sprites, build a mesh + material and tag with `Mesh2d` / `MeshMaterial2d`:

```rust
fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    commands.spawn(Camera2d);
    commands.spawn((
        Mesh2d(meshes.add(Circle::new(50.0))),
        MeshMaterial2d(materials.add(Color::hsl(120.0, 0.9, 0.6))),
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));
}
```

Shape primitives include `Circle`, `Rectangle`, `Ellipse`, `Capsule2d`, `RegularPolygon`, `Triangle2d`, `Annulus`, and more; each converts into a `Mesh` via `meshes.add(...)`.

## Input (for interactive scenes)

```rust
fn keys(input: Res<ButtonInput<KeyCode>>) {
    if input.pressed(KeyCode::ArrowRight) { /* held */ }
    if input.just_pressed(KeyCode::Space) { /* edge */ }
    if input.just_released(KeyCode::KeyA) { /* edge */ }
}
```

## Hierarchy

Parent/child entities use the `children!` macro at spawn:

```rust
commands.spawn((
    Sprite::from_color(Color::WHITE, Vec2::splat(40.0)),
    Transform::default(),
    children![(
        Sprite::from_color(Color::BLACK, Vec2::splat(10.0)),
        Transform::from_xyz(0.0, 20.0, 1.0),   // relative to parent
    )],
));
```

Child `Transform`s are relative to the parent.
