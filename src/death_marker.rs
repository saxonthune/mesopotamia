//! Death markers — a red X that blooms where an elk starves and fades away.
//!
//! Purely observational render feedback: each `Starved` event (events.rs) drops a
//! marker on the cell the elk died in — two crossed red rectangles, flat-coloured,
//! no texture and no outline — at full red, fading to nothing over `LIFETIME` ticks.
//! Nothing in the simulation reads it; it only makes the herd's losses legible to
//! watch. The fade curve is the one decision worth pinning, so it is a pure tested
//! function (`marker_alpha`); the systems only spawn and age sprites around it.

use bevy::prelude::*;

use crate::elk::Spawner;
use crate::events::{EventKind, EventLog};
use crate::grid::Grid;
use crate::render::{cell_world_pos, TILE_SIZE};

/// Ticks a marker takes to fade from full red to gone.
const LIFETIME: u32 = 40;
/// Long axis of each bar, as a share of the tile — the X spans most of the cell.
const BAR_LEN: f32 = TILE_SIZE * 0.62;
/// Short axis (thickness) of each bar.
const BAR_THICK: f32 = TILE_SIZE * 0.16;
/// Z above the terrain layers (grass 0.3 … flower 0.7) but below the live elk (1.0).
const MARKER_Z: f32 = 0.9;

/// One bar of a death-X. Stores the sim tick it was born on so the fader can age
/// it; both bars of a death share the same `born` tick and fade together.
#[derive(Component)]
struct DeathMark {
    born: u32,
}

/// Cursor into `EventLog.total`: how many events have been turned into markers, so
/// each `Starved` spawns its X exactly once even as the ring evicts old entries.
#[derive(Resource, Default)]
struct MarkerCursor(u64);

/// Alpha of a marker `age` ticks into a `lifetime`-tick fade: full (1.0) at birth,
/// linearly to 0.0 at `lifetime` and beyond. A zero lifetime reads as already gone.
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

/// Drop a fresh X on every `Starved` cell pushed since last tick. Mirrors the
/// score system's cursor walk so each death is marked once; departures are wins,
/// not deaths, so they are skipped.
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
        // Two bars crossed at ±45° make the X. No outline, no texture — just the
        // flat red rectangles, which read as crisp pixels at this tile scale.
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

/// Age every marker against the sim clock: tint its red down the fade curve, and
/// despawn it once spent so dead markers don't accumulate.
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

    // Full red at birth, gone at the end of the fade — the two endpoints the marker
    // is built around.
    #[test]
    fn alpha_runs_full_to_zero() {
        assert!((marker_alpha(0, 40) - 1.0).abs() < 1e-6, "fresh marker is full red");
        assert!((marker_alpha(40, 40)).abs() < 1e-6, "spent marker is gone");
    }

    // Monotone decreasing: a marker never brightens as it ages. Breaking input —
    // if the fade rose with age this would fail.
    #[test]
    fn alpha_never_rises_with_age() {
        let mut prev = marker_alpha(0, 40);
        for age in 1..=40 {
            let a = marker_alpha(age, 40);
            assert!(a <= prev, "alpha rose at age {age}: {a} > {prev}");
            prev = a;
        }
    }

    // Past the lifetime it stays clamped at zero, and a zero lifetime is gone at once.
    #[test]
    fn alpha_clamps_past_lifetime() {
        assert_eq!(marker_alpha(100, 40), 0.0, "an over-age marker stays gone");
        assert_eq!(marker_alpha(0, 0), 0.0, "a zero-lifetime marker is gone immediately");
    }

    // Half-life: at the midpoint the marker is at half alpha (the linear curve).
    #[test]
    fn alpha_is_half_at_midpoint() {
        assert!((marker_alpha(20, 40) - 0.5).abs() < 1e-6);
    }
}
