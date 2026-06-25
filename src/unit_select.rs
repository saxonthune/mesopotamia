//! Box-select unit inspector: a StarCraft-style group selection over the map
//! view. Left-drag rubber-bands a rectangle, capturing the individual elk inside
//! it into a roster; the roster, a per-unit stats readout, and a tinted portrait
//! render in an auto-shown "Selection" tab of the bottom dock (drawn by `ui`).
//!
//! This is a second, independent selection axis from `pick_herd`'s single-click
//! herd pick — it operates on individual elk entities and is *read-only* over elk
//! components, so it never collides with the simulation. The module owns the
//! input gesture, the rubber-band overlay, the selection state, and the shared
//! silhouette art; the three-column tab view lives in `ui` beside the egui
//! helpers it reuses.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};

use crate::elk::{elk_color, Elk, Herding};
use crate::render::{cell_world_pos, WorldCamera};

/// Pointer travel (in logical points) past which a left-press becomes a box drag
/// rather than a click. Below it the gesture falls through to `pick_herd`.
const DRAG_THRESHOLD: f32 = 4.0;

/// Cap on how many elk a single box captures. A box over a dense field keeps the
/// first `MAX_UNITS` in query order (no sort) and reports the true total; this
/// bounds the thumbnail grid so the roster stays scannable.
const MAX_UNITS: usize = 96;

/// The live box-selection. Read by `ui`'s Selection tab and by `pick_herd`
/// (which skips the click that ended a drag). All fields default empty/idle.
#[derive(Resource, Default)]
pub struct UnitSelectState {
    /// Box-captured elk, in capture (query) order, length ≤ `MAX_UNITS`.
    pub units: Vec<Entity>,
    /// The thumbnail clicked into the stats + portrait columns.
    pub focused: Option<Entity>,
    /// Total elk the last box actually contained, before the `MAX_UNITS` cap —
    /// drives the "96 of N" roster header.
    pub captured_total: usize,
    /// Edge flag: a non-empty capture happened this frame. `ui` consumes it to
    /// auto-open the Selection tab, then clears it.
    pub just_selected: bool,
    /// Screen-space press anchor while the left button is held over the world;
    /// `None` when idle or when the press landed on the UI.
    anchor: Option<Vec2>,
    /// The current gesture has crossed `DRAG_THRESHOLD` — it is a box, not a click.
    is_box: bool,
    /// The gesture that released *this frame* was a box. `pick_herd` reads this to
    /// avoid picking a herd on the click that closed a drag.
    pub drag_was_box: bool,
}

/// The selection resource plus the elk query, bundled so `control_panel` can take
/// the whole feature as one system param and stay under Bevy's arity limit.
#[derive(bevy::ecs::system::SystemParam)]
pub struct SelectionParams<'w, 's> {
    pub state: ResMut<'w, UnitSelectState>,
    pub elk: Query<'w, 's, (Entity, &'static Elk, &'static Herding)>,
}

pub struct UnitSelectPlugin;

impl Plugin for UnitSelectPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UnitSelectState>()
            // box_select must set `drag_was_box` before pick_herd reads it.
            .add_systems(Update, box_select.before(crate::ui::pick_herd))
            .add_systems(EguiPrimaryContextPass, draw_selection_box);
    }
}

/// Collect up to `cap` ids whose position lies within the inclusive rect
/// `[min, max]`, in iteration order. Returns the kept ids and the *total* number
/// in the rect (which may exceed `cap`). Pure over its inputs so the capture and
/// truncation rule is unit-tested without a world.
pub fn capture_in_rect<T>(
    items: impl IntoIterator<Item = (T, Vec2)>,
    min: Vec2,
    max: Vec2,
    cap: usize,
) -> (Vec<T>, usize) {
    let mut kept = Vec::new();
    let mut total = 0usize;
    for (id, p) in items {
        if p.x >= min.x && p.x <= max.x && p.y >= min.y && p.y <= max.y {
            total += 1;
            if kept.len() < cap {
                kept.push(id);
            }
        }
    }
    (kept, total)
}

/// Drive the box-select gesture: arm on a world press, become a box once the
/// pointer travels past the threshold, and on release either capture the enclosed
/// elk (a drag) or dismiss the current group (a plain click). Escape clears too.
pub fn box_select(
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut contexts: EguiContexts,
    window: Single<&Window>,
    camera: Single<(&Camera, &GlobalTransform), With<WorldCamera>>,
    grid: Res<crate::grid::Grid>,
    elk: Query<(Entity, &Elk)>,
    mut sel: ResMut<UnitSelectState>,
) -> Result {
    let over_ui = contexts.ctx_mut()?.wants_pointer_input();
    let cursor = window.cursor_position();

    if keys.just_pressed(KeyCode::Escape) {
        sel.units.clear();
        sel.focused = None;
        sel.anchor = None;
        sel.is_box = false;
    }

    // Arm a potential box only when the press lands on the world, not the panel.
    if mouse.just_pressed(MouseButton::Left) && !over_ui {
        sel.anchor = cursor;
        sel.is_box = false;
    }

    // Promote to a box once the pointer has travelled far enough from the anchor.
    if mouse.pressed(MouseButton::Left) {
        if let (Some(anchor), Some(cur)) = (sel.anchor, cursor) {
            if (cur - anchor).length() > DRAG_THRESHOLD {
                sel.is_box = true;
            }
        }
    }

    sel.drag_was_box = false;
    if mouse.just_released(MouseButton::Left) {
        if sel.is_box {
            sel.drag_was_box = true;
            let (cam, cam_tf) = *camera;
            if let (Some(anchor), Some(release)) = (sel.anchor, cursor) {
                // Unproject both corners through the world camera so the rect
                // respects the viewport clip above the dock (as pick_herd does).
                if let (Ok(a), Ok(b)) = (
                    cam.viewport_to_world_2d(cam_tf, anchor),
                    cam.viewport_to_world_2d(cam_tf, release),
                ) {
                    let min = a.min(b);
                    let max = a.max(b);
                    let items = elk.iter().map(|(e, elk)| (e, cell_world_pos(&grid, elk.cell)));
                    let (units, total) = capture_in_rect(items, min, max, MAX_UNITS);
                    sel.focused = None;
                    if total > 0 {
                        sel.units = units;
                        sel.captured_total = total;
                        sel.just_selected = true;
                    } else {
                        sel.units.clear();
                    }
                }
            }
        } else if sel.anchor.is_some() {
            // A plain click on the world dismisses the current group, the way a
            // StarCraft left-click deselects. The herd pick is pick_herd's job.
            sel.units.clear();
            sel.focused = None;
        }
        sel.anchor = None;
        sel.is_box = false;
    }
    Ok(())
}

/// Paint the rubber-band rectangle while a box drag is in progress. Drawn on a
/// foreground egui layer so it floats over the world without claiming layout.
fn draw_selection_box(
    mut contexts: EguiContexts,
    window: Single<&Window>,
    sel: Res<UnitSelectState>,
) -> Result {
    if !sel.is_box {
        return Ok(());
    }
    let (Some(anchor), Some(cur)) = (sel.anchor, window.cursor_position()) else {
        return Ok(());
    };
    let ctx = contexts.ctx_mut()?;
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("unit_box_select"),
    ));
    let rect = egui::Rect::from_two_pos(
        egui::pos2(anchor.x, anchor.y),
        egui::pos2(cur.x, cur.y),
    );
    painter.rect_filled(rect, 0.0, egui::Color32::from_rgba_unmultiplied(120, 200, 120, 24));
    painter.rect_stroke(
        rect,
        0.0,
        egui::Stroke::new(1.5, egui::Color32::from_rgb(120, 200, 120)),
        egui::StrokeKind::Middle,
    );
    Ok(())
}

/// The herd tint as an egui colour, matching the map sprite (`sync_elk_color`).
pub fn herd_color32(slot: u8) -> egui::Color32 {
    let c = elk_color(slot as usize, false).to_srgba();
    egui::Color32::from_rgb(
        (c.red * 255.0) as u8,
        (c.green * 255.0) as u8,
        (c.blue * 255.0) as u8,
    )
}

/// The shared programmer-art elk silhouette, tinted by herd colour, filling
/// `rect`. One drawing reused at thumbnail and portrait scale — the "single
/// image" placeholder. Swap real art in here when it lands; callers are unchanged.
pub fn draw_elk_silhouette(painter: &egui::Painter, rect: egui::Rect, color: egui::Color32) {
    painter.rect_filled(rect, 4.0, egui::Color32::from_gray(28));
    let c = rect.center();
    let s = rect.width().min(rect.height());
    // Body and head.
    painter.circle_filled(c + egui::vec2(-s * 0.04, s * 0.06), s * 0.26, color);
    let head = c + egui::vec2(s * 0.22, -s * 0.16);
    painter.circle_filled(head, s * 0.12, color);
    // Antlers — a couple of forking strokes off the head.
    let antler = egui::Stroke::new((s * 0.03).max(1.0), color);
    painter.line_segment([head, head + egui::vec2(s * 0.10, -s * 0.22)], antler);
    painter.line_segment([head, head + egui::vec2(-s * 0.02, -s * 0.26)], antler);
    painter.line_segment(
        [head + egui::vec2(s * 0.04, -s * 0.13), head + egui::vec2(s * 0.16, -s * 0.17)],
        antler,
    );
    // Legs.
    let leg = egui::Stroke::new((s * 0.045).max(1.0), color);
    for dx in [-0.14_f32, -0.02, 0.10] {
        let x = c.x + dx * s;
        painter.line_segment(
            [egui::pos2(x, c.y + s * 0.14), egui::pos2(x, c.y + s * 0.34)],
            leg,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(x: f32, y: f32) -> Vec2 {
        Vec2::new(x, y)
    }

    #[test]
    fn captures_points_inside_inclusive_rect() {
        let items = vec![(0u32, at(0.0, 0.0)), (1, at(5.0, 5.0)), (2, at(10.0, 10.0))];
        let (kept, total) = capture_in_rect(items, at(0.0, 0.0), at(10.0, 10.0), 64);
        assert_eq!(kept, vec![0, 1, 2]); // boundary points included
        assert_eq!(total, 3);
    }

    #[test]
    fn excludes_points_outside_rect() {
        let items = vec![(0u32, at(-1.0, 0.0)), (1, at(5.0, 5.0)), (2, at(11.0, 5.0))];
        let (kept, total) = capture_in_rect(items, at(0.0, 0.0), at(10.0, 10.0), 64);
        assert_eq!(kept, vec![1]);
        assert_eq!(total, 1);
    }

    #[test]
    fn truncates_to_cap_keeping_query_order() {
        let items: Vec<(u32, Vec2)> = (0..10).map(|i| (i, at(1.0, 1.0))).collect();
        let (kept, total) = capture_in_rect(items, at(0.0, 0.0), at(2.0, 2.0), 4);
        assert_eq!(kept, vec![0, 1, 2, 3]); // first-N in order, no sort
        assert_eq!(total, 10); // true total survives the cap
    }

    #[test]
    fn empty_when_nothing_inside() {
        let items = vec![(0u32, at(100.0, 100.0))];
        let (kept, total) = capture_in_rect(items, at(0.0, 0.0), at(10.0, 10.0), 64);
        assert!(kept.is_empty());
        assert_eq!(total, 0);
    }
}
