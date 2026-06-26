//! Red X markers on starvation cells, fading over `LIFETIME` ticks. Render-only; nothing in the sim reads them.

use bevy::prelude::*;

use crate::elk::Spawner;
use crate::events::{EventKind, EventLog};
use crate::grid::Grid;
use crate::render::{cell_world_pos, TILE_SIZE};

const LIFETIME: u32 = 120;
const BAR_LEN: f32 = TILE_SIZE * std::f32::consts::SQRT_2;
const BAR_THICK: f32 = TILE_SIZE * 0.16;
// Above terrain layers (grass 0.3…flower 0.7), below live elk (1.0).
const MARKER_Z: f32 = 0.9;

#[derive(Component)]
struct DeathMark {
    born: u32,
}

// Tracks consumed event count; prevents re-marking when the ring evicts old entries.
#[derive(Resource, Default)]
struct MarkerCursor(u64);

pub fn marker_alpha(age: u32, lifetime: u32) -> f32 {
    if lifetime == 0 {
        return 0.0;
    }
    (1.0 - age as f32 / lifetime as f32).clamp(0.0, 1.0)
}

pub struct DeathMarkerPlugin;

impl Plugin for DeathMarkerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MarkerCursor>()
            .add_systems(FixedUpdate, (spawn_markers, fade_markers).chain());
    }
}

fn spawn_markers(
    mut commands: Commands,
    mut cursor: ResMut<MarkerCursor>,
    events: Res<EventLog>,
    grid: Res<Grid>,
) {
    let new = events.total.saturating_sub(cursor.0) as usize;
    if new == 0 {
        return;
    }
    let take = new.min(events.recent.len());
    let start = events.recent.len() - take;
    for e in events.recent.iter().skip(start) {
        if !matches!(e.kind, EventKind::Starved) {
            continue;
        }
        let pos = cell_world_pos(&grid, e.cell);
        for angle in [std::f32::consts::FRAC_PI_4, -std::f32::consts::FRAC_PI_4] {
            commands.spawn((
                Sprite::from_color(Color::srgb(1.0, 0.0, 0.0), Vec2::new(BAR_LEN, BAR_THICK)),
                Transform::from_xyz(pos.x, pos.y, MARKER_Z)
                    .with_rotation(Quat::from_rotation_z(angle)),
                DeathMark { born: e.tick as u32 },
            ));
        }
    }
    cursor.0 = events.total;
}

fn fade_markers(
    mut commands: Commands,
    spawner: Res<Spawner>,
    mut marks: Query<(Entity, &DeathMark, &mut Sprite)>,
) {
    let now = spawner.elapsed;
    for (entity, mark, mut sprite) in &mut marks {
        let age = now.saturating_sub(mark.born);
        if age >= LIFETIME {
            commands.entity(entity).despawn();
        } else {
            sprite.color.set_alpha(marker_alpha(age, LIFETIME));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alpha_runs_full_to_zero() {
        assert!((marker_alpha(0, 40) - 1.0).abs() < 1e-6, "fresh marker is full red");
        assert!((marker_alpha(40, 40)).abs() < 1e-6, "spent marker is gone");
    }

    #[test]
    fn alpha_never_rises_with_age() {
        let mut prev = marker_alpha(0, 40);
        for age in 1..=40 {
            let a = marker_alpha(age, 40);
            assert!(a <= prev, "alpha rose at age {age}: {a} > {prev}");
            prev = a;
        }
    }

    #[test]
    fn alpha_clamps_past_lifetime() {
        assert_eq!(marker_alpha(100, 40), 0.0, "an over-age marker stays gone");
        assert_eq!(marker_alpha(0, 0), 0.0, "a zero-lifetime marker is gone immediately");
    }

    #[test]
    fn alpha_is_half_at_midpoint() {
        assert!((marker_alpha(20, 40) - 0.5).abs() < 1e-6);
    }
}
