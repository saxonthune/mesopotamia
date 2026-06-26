//! Box-select: left-drag to capture elk into a roster shown in the Selection tab (drawn by `ui`).
//! Independent of `pick_herd` — read-only over elk components, never collides with the simulation.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};

use crate::elk::{elk_color, Elk, Herding};
use crate::render::{cell_world_pos, WorldCamera};

/// Below this, a left-press falls through to `pick_herd` as a click.
const DRAG_THRESHOLD: f32 = 4.0;

/// Box captures the first N in iteration order (no sort); true total still reported.
const MAX_UNITS: usize = 96;

/// Live box-selection state. `pick_herd` reads `drag_was_box` to skip the click that ended a drag.
#[derive(Resource, Default)]
pub struct UnitSelectState {
    /// In capture order, length ≤ `MAX_UNITS`.
    pub units: Vec<Entity>,
    pub focused: Option<Entity>,
    /// True total from the last box, before the `MAX_UNITS` cap — drives the "96 of N" header.
    pub captured_total: usize,
    /// Edge flag: `ui` consumes this to auto-open the Selection tab, then clears it.
    pub just_selected: bool,
    /// Press anchor in screen-space; `None` when idle or press landed on UI.
    anchor: Option<Vec2>,
    is_box: bool,
    /// The gesture that released *this frame* was a box. `pick_herd` reads this to
    /// avoid picking a herd on the click that closed a drag.
    pub drag_was_box: bool,
}

/// Bundled so `control_panel` stays under Bevy's system-param arity limit.
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

/// Pure fn: keeps up to `cap` items in iteration order; also returns the true total (may exceed `cap`).
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
                // Unproject both corners through the world camera to respect the viewport clip above the dock.
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
            // Plain click on world dismisses the group; herd pick is pick_herd's job.
            sel.units.clear();
            sel.focused = None;
        }
        sel.anchor = None;
        sel.is_box = false;
    }
    Ok(())
}

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

/// Matches the map sprite tint from `sync_elk_color`.
pub fn herd_color32(slot: u8) -> egui::Color32 {
    let c = elk_color(slot as usize, false).to_srgba();
    egui::Color32::from_rgb(
        (c.red * 255.0) as u8,
        (c.green * 255.0) as u8,
        (c.blue * 255.0) as u8,
    )
}

pub fn draw_elk_silhouette(painter: &egui::Painter, rect: egui::Rect, color: egui::Color32) {
    painter.rect_filled(rect, 4.0, egui::Color32::from_gray(28));
    let c = rect.center();
    let s = rect.width().min(rect.height());
    painter.circle_filled(c + egui::vec2(-s * 0.04, s * 0.06), s * 0.26, color);
    let head = c + egui::vec2(s * 0.22, -s * 0.16);
    painter.circle_filled(head, s * 0.12, color);
    let antler = egui::Stroke::new((s * 0.03).max(1.0), color);
    painter.line_segment([head, head + egui::vec2(s * 0.10, -s * 0.22)], antler);
    painter.line_segment([head, head + egui::vec2(-s * 0.02, -s * 0.26)], antler);
    painter.line_segment(
        [head + egui::vec2(s * 0.04, -s * 0.13), head + egui::vec2(s * 0.16, -s * 0.17)],
        antler,
    );
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
        assert_eq!(kept, vec![0, 1, 2]);
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
