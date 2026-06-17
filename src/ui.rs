use bevy::prelude::*;
use bevy::camera::Viewport;
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};

use crate::elk::{Elk, ElkParams, Herds};
use crate::grid::{Fertility, Grid, GrowthRate};
use crate::render::{cell_world_pos, CameraSettings, WorldCamera};

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiState>()
            // pick_herd (world click → select) runs before the panel draws it.
            .add_systems(Update, pick_herd)
            .add_systems(
                EguiPrimaryContextPass,
                // control_panel lays out the dock; set_camera_viewport then reads
                // the leftover rect, so order matters — hence .chain().
                (control_panel, set_camera_viewport).chain(),
            );
    }
}

#[derive(Resource, Default)]
struct UiState {
    tab: Tab,
    selected: Option<u32>, // hex code of the herd shown in the details pane;
    // persists on a dead/migrated cohort until the user picks another.
}

#[derive(Default, PartialEq, Clone, Copy)]
enum Tab {
    #[default]
    Sliders,
    Herds,
}

/// Standard labelled `f32` slider — the one-liner every control page uses.
fn slider(ui: &mut egui::Ui, value: &mut f32, range: std::ops::RangeInclusive<f32>, label: &str) {
    ui.add(egui::Slider::new(value, range).text(label));
}

/// One self-contained control group: a heading and its body. The body is a
/// closure so a panel is just a titled `&mut Ui` consumer — the immediate-mode
/// analogue of a composed component.
struct Panel<'a> {
    title: &'a str,
    /// Target column width — the flex-basis. Panels wrap to a new row when the
    /// current row can't fit another at its width.
    width: f32,
    body: Box<dyn FnOnce(&mut egui::Ui) + 'a>,
}

impl<'a> Panel<'a> {
    const DEFAULT_WIDTH: f32 = 240.0;

    fn new(title: &'a str, body: impl FnOnce(&mut egui::Ui) + 'a) -> Self {
        Self { title, width: Self::DEFAULT_WIDTH, body: Box::new(body) }
    }

    /// Override the flex-basis for a wider/narrower group.
    fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }
}

/// Lay panels out left-to-right, wrapping to a new row when the row fills —
/// `flex-wrap: wrap`. Each panel is boxed to its own `width` so it forms a
/// column rather than spreading to fill the row.
fn panel_flow(ui: &mut egui::Ui, panels: Vec<Panel>) {
    ui.horizontal_wrapped(|ui| {
        for p in panels {
            ui.allocate_ui_with_layout(
                egui::vec2(p.width, ui.available_height()),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        ui.set_width(p.width);
                        ui.heading(p.title);
                        (p.body)(ui);
                    });
                },
            );
        }
    });
}

/// The docked bottom panel: a tab bar with always-visible speed controls, and a
/// scrolling content area paged by the selected tab.
#[allow(clippy::too_many_arguments)]
fn control_panel(
    mut contexts: EguiContexts,
    mut state: ResMut<UiState>,
    mut elk_params: ResMut<ElkParams>,
    mut growth: ResMut<GrowthRate>,
    mut fertility: ResMut<Fertility>,
    mut camera: ResMut<CameraSettings>,
    mut time: ResMut<Time<Virtual>>,
    herds: Res<Herds>,
) -> Result {
    egui::TopBottomPanel::bottom("control_panel")
        .resizable(true)
        .default_height(280.0)
        .min_height(120.0)
        .show(contexts.ctx_mut()?, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut state.tab, Tab::Sliders, "Sliders");
                ui.selectable_value(&mut state.tab, Tab::Herds, "Herds");
                ui.separator();
                speed_inline(ui, time.as_mut());
            });
            ui.separator();
            // The panel height is user-set by dragging its top border; the
            // scroll area fills whatever is left below the tab bar, so a taller
            // panel reveals more rows and a short one scrolls.
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| match state.tab {
                Tab::Sliders => panel_flow(ui, vec![
                    Panel::new("Grass", |ui| grass_tab(ui, growth.as_mut(), fertility.as_mut())),
                    // Behaviour carries the most rows, so give it a wider column.
                    Panel::new("Behaviour", |ui| behaviour_tab(ui, elk_params.as_mut())).width(300.0),
                    Panel::new("View", |ui| view_tab(ui, camera.as_mut())),
                ]),
                Tab::Herds => herds_view(ui, state.as_mut(), &herds),
            });
        });
    Ok(())
}

/// The Herds tab: a scrollable list of herd cards on the left; clicking one
/// shows its full details in the scrollable pane on the right. The selection
/// persists on a dead/migrated cohort until the user picks another.
fn herds_view(ui: &mut egui::Ui, state: &mut UiState, herds: &Herds) {
    let alive: Vec<u32> = herds
        .order
        .iter()
        .copied()
        .filter(|c| herds.cohorts.get(c).is_some_and(|co| co.alive > 0))
        .collect();

    ui.columns(2, |cols| {
        egui::ScrollArea::vertical()
            .id_salt("herd_cards")
            .show(&mut cols[0], |ui| {
                if alive.is_empty() {
                    ui.weak("no herds");
                }
                for &code in &alive {
                    if let Some(co) = herds.cohorts.get(&code) {
                        herd_card(ui, code, co, state);
                    }
                }
            });

        egui::ScrollArea::vertical()
            .id_salt("herd_details")
            .show(&mut cols[1], |ui| match state
                .selected
                .and_then(|c| herds.cohorts.get(&c).map(|co| (c, co)))
            {
                Some((code, co)) => herd_details(ui, code, co),
                None => {
                    ui.weak("select a herd");
                }
            });
    });
}

fn herd_health(co: &crate::elk::Cohort) -> f32 {
    if co.alive > 0 {
        co.energy_sum / co.alive as f32
    } else {
        0.0
    }
}

/// A clickable summary card for one herd. Highlights when selected.
fn herd_card(ui: &mut egui::Ui, code: u32, co: &crate::elk::Cohort, state: &mut UiState) {
    let mut frame = egui::Frame::group(ui.style());
    if state.selected == Some(code) {
        frame = frame.fill(ui.visuals().selection.bg_fill);
    }
    let resp = frame
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.strong(format!("{code:06x}"));
                if co.alive == 0 {
                    ui.weak("· gone");
                }
            });
            ui.add(
                egui::ProgressBar::new(herd_health(co))
                    .desired_width(ui.available_width())
                    .text(format!("health {:.0}%", herd_health(co) * 100.0)),
            );
            ui.label(format!("alive {} · peak {} · slot {}", co.alive, co.peak, co.slot));
        })
        .response
        .interact(egui::Sense::click());

    if resp.clicked() {
        state.selected = Some(code);
    }
}

fn herd_details(ui: &mut egui::Ui, code: u32, co: &crate::elk::Cohort) {
    ui.heading(format!("pack {code:06x}"));
    ui.label(format!("status: {}", if co.alive > 0 { "alive" } else { "gone" }));
    ui.label(format!("slot: {}", co.slot));
    ui.separator();
    ui.add(
        egui::ProgressBar::new(herd_health(co))
            .text(format!("avg energy {:.0}%", herd_health(co) * 100.0)),
    );
    ui.label(format!("alive: {}", co.alive));
    ui.label(format!("peak: {}", co.peak));
    ui.label(format!("starved: {}", co.deaths));
    ui.label(format!("migrated out: {}", co.departures));
}

/// A left-click in the world selects the nearest elk's herd and opens the Herds
/// tab on it. Clicks over the egui panel are ignored. The cursor is unprojected
/// through the world camera, so it respects the viewport clip above the dock.
fn pick_herd(
    mouse: Res<ButtonInput<MouseButton>>,
    mut contexts: EguiContexts,
    window: Single<&Window>,
    camera: Single<(&Camera, &GlobalTransform), With<WorldCamera>>,
    grid: Res<Grid>,
    elk: Query<&Elk>,
    mut state: ResMut<UiState>,
) -> Result {
    if !mouse.just_pressed(MouseButton::Left) {
        return Ok(());
    }
    if contexts.ctx_mut()?.wants_pointer_input() {
        return Ok(()); // the click landed on the UI
    }
    let Some(cursor) = window.cursor_position() else {
        return Ok(());
    };
    // Let the camera do the unprojection — it knows its own viewport, so a click
    // maps to the right world point even though the world is clipped to the area
    // above the dock. Outside the viewport (e.g. over the panel) this errors.
    let (cam, cam_transform) = *camera;
    let Ok(world) = cam.viewport_to_world_2d(cam_transform, cursor) else {
        return Ok(());
    };

    // Nearest elk within a generous world-space radius (~6 tiles) picks the herd.
    const PICK_RADIUS: f32 = 96.0;
    let tol2 = PICK_RADIUS * PICK_RADIUS;
    let mut best: Option<(u32, f32)> = None;
    for elk in &elk {
        let d2 = (cell_world_pos(&grid, elk.cell) - world).length_squared();
        if d2 <= tol2 && best.is_none_or(|(_, b)| d2 < b) {
            best = Some((elk.code, d2));
        }
    }
    if let Some((code, _)) = best {
        state.selected = Some(code);
        state.tab = Tab::Herds;
    }
    Ok(())
}

/// Map the egui logical rect left free above the bottom dock to a physical-pixel
/// camera viewport, so the world renders above the panel rather than behind it.
///
/// Pure so it can be unit-tested without a window: `min`/`size` are the egui
/// available rect in logical points, `scale` is the window's scale factor, and
/// `target` is the render target's physical size. Returns `None` when the free
/// area collapses (the panel fills the window) so the caller clears the
/// viewport and the camera falls back to the whole window.
fn world_viewport(min: Vec2, size: Vec2, scale: f32, target: UVec2) -> Option<Viewport> {
    let pos = (min * scale).max(Vec2::ZERO).as_uvec2();
    let px = (size * scale).max(Vec2::ZERO).as_uvec2();
    // Clamp so position + size never exceed the target (an over-large scissor
    // rect is a hard wgpu crash, not a clip).
    let w = px.x.min(target.x.saturating_sub(pos.x));
    let h = px.y.min(target.y.saturating_sub(pos.y));
    if w == 0 || h == 0 {
        return None;
    }
    Some(Viewport {
        physical_position: pos,
        physical_size: UVec2::new(w, h),
        ..default()
    })
}

/// Confine the *world* camera to the rect egui leaves above the dock. The egui
/// camera is separate and full-window, so clipping here never touches the UI.
fn set_camera_viewport(
    mut contexts: EguiContexts,
    window: Single<&Window>,
    camera: Single<&mut Camera, With<WorldCamera>>,
) -> Result {
    let avail = contexts.ctx_mut()?.available_rect(); // free area above the panel
    let target = UVec2::new(window.physical_width(), window.physical_height());
    camera.into_inner().viewport = world_viewport(
        Vec2::new(avail.min.x, avail.min.y),
        Vec2::new(avail.width(), avail.height()),
        window.scale_factor(),
        target,
    );
    Ok(())
}

fn speed_inline(ui: &mut egui::Ui, time: &mut Time<Virtual>) {
    const PRESETS: [(&str, f32); 5] =
        [("1x", 1.0), ("2x", 2.0), ("3x", 3.0), ("4x", 4.0), (">>", 64.0)];
    let current = time.relative_speed();
    if ui.selectable_label(time.is_paused(), "‖").clicked() {
        if time.is_paused() {
            time.unpause();
        } else {
            time.pause();
        }
    }
    for (label, mult) in PRESETS {
        if ui.selectable_label(current == mult, label).clicked() {
            time.set_relative_speed(mult);
        }
    }
    let mut custom = current;
    if ui
        .add(egui::DragValue::new(&mut custom).range(0.0..=256.0).speed(0.1))
        .changed()
    {
        time.set_relative_speed(custom);
    }
}

fn grass_tab(ui: &mut egui::Ui, growth: &mut GrowthRate, fertility: &mut Fertility) {
    slider(ui, &mut growth.intrinsic, -0.01..=0.1, "regrowth / tick");
    slider(ui, &mut growth.spread, 0.0..=0.3, "spread from neighbours");
    slider(ui, &mut fertility.rate, 0.0..=0.5, "poop → grass / tick");
    slider(ui, &mut fertility.efficiency, 0.0..=1.0, "conversion efficiency");
}

fn behaviour_tab(ui: &mut egui::Ui, p: &mut ElkParams) {
    ui.label("drive weights");
    slider(ui, &mut p.separation, 0.0..=3.0, "separation");
    slider(ui, &mut p.cohesion, 0.0..=3.0, "cohesion");
    slider(ui, &mut p.grass, 0.0..=3.0, "grass-seeking");
    slider(ui, &mut p.social, 0.0..=3.0, "social foraging");
    slider(ui, &mut p.migration, 0.0..=2.0, "migration (fallback)");
    ui.separator();
    ui.label("perception (cells)");
    slider(ui, &mut p.sep_radius, 1.0..=10.0, "separation radius");
    slider(ui, &mut p.coh_radius, 1.0..=20.0, "cohesion radius");
    slider(ui, &mut p.grass_radius, 1.0..=12.0, "grass radius");
    slider(ui, &mut p.social_radius, 1.0..=40.0, "social radius");
    slider(ui, &mut p.temperature, 0.1..=2.0, "step randomness");
    ui.separator();
    ui.label("metabolism");
    slider(ui, &mut p.bite, 0.0..=1.0, "bite / graze");
    // Ranged near `energy_drain` — energy in must beat energy out, so the whole
    // live/die regime sits within a few × of the drain, not out at 1.0.
    slider(ui, &mut p.energy_per_bite, 0.0..=0.05, "energy / bite");
    slider(ui, &mut p.energy_drain, 0.0..=0.02, "energy drain / tick");
    // The balance made legible: net energy/tick = energy_per_bite·g − energy_drain,
    // so an elk lives only if it grazes at least drain/energy_per_bite of the time.
    ui.label(if p.energy_per_bite <= 0.0 {
        "break-even grazing: impossible (no energy/bite)".to_string()
    } else {
        let pct = p.energy_drain / p.energy_per_bite * 100.0;
        if pct > 100.0 {
            format!("break-even grazing: {pct:.0}% — herds starve")
        } else {
            format!("break-even grazing: {pct:.0}% of ticks")
        }
    });
    ui.separator();
    ui.label("browse & crossing");
    slider(ui, &mut p.browse_energy, 0.0..=0.1, "energy / browse bite");
    slider(ui, &mut p.browse_bite, 0.0..=1.0, "browse / bite");
    slider(ui, &mut p.water_cost, 0.0..=4.0, "water crossing cost");
    slider(ui, &mut p.ford_discount, 0.0..=1.0, "ford discount (0 = free)");
    slider(ui, &mut p.swim_drain, 0.0..=0.05, "swim energy drain");
    slider(ui, &mut p.mig_growth, 0.0..=0.01, "migration growth / tick");
}

fn view_tab(ui: &mut egui::Ui, cam: &mut CameraSettings) {
    ui.add(
        egui::Slider::new(&mut cam.zoom, 0.1..=10.0)
            .logarithmic(true)
            .text("zoom"),
    );
    slider(ui, &mut cam.pan.x, -2000.0..=2000.0, "pan x");
    slider(ui, &mut cam.pan.y, -2000.0..=2000.0, "pan y");
    if ui.button("reset view").clicked() {
        cam.zoom = 1.0;
        cam.pan = Vec2::ZERO;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A 1280x960 window at scale 1 with no panel: the world fills it.
    #[test]
    fn full_window_when_no_panel() {
        let vp = world_viewport(Vec2::ZERO, Vec2::new(1280.0, 960.0), 1.0, UVec2::new(1280, 960))
            .expect("non-empty viewport");
        assert_eq!(vp.physical_position, UVec2::ZERO);
        assert_eq!(vp.physical_size, UVec2::new(1280, 960));
    }

    // A 240pt dock at the bottom shrinks only the height; the world stays pinned
    // to the top-left.
    #[test]
    fn bottom_dock_shrinks_height_only() {
        let vp = world_viewport(Vec2::ZERO, Vec2::new(1280.0, 720.0), 1.0, UVec2::new(1280, 960))
            .expect("non-empty viewport");
        assert_eq!(vp.physical_position, UVec2::ZERO);
        assert_eq!(vp.physical_size, UVec2::new(1280, 720));
    }

    // The Retina regression: logical points must be scaled to physical pixels
    // exactly once. A 640x360 logical region at scale 2 is 1280x720 physical —
    // not 2560x1440 (double-counted, which crashed wgpu with an oversized
    // scissor rect).
    #[test]
    fn retina_scale_counts_once() {
        let vp = world_viewport(Vec2::ZERO, Vec2::new(640.0, 360.0), 2.0, UVec2::new(1280, 720))
            .expect("non-empty viewport");
        assert_eq!(vp.physical_size, UVec2::new(1280, 720));
    }

    // A rect larger than the target is clamped, never allowed to exceed it.
    #[test]
    fn oversize_rect_clamps_to_target() {
        let vp = world_viewport(Vec2::ZERO, Vec2::new(4000.0, 4000.0), 1.0, UVec2::new(1280, 720))
            .expect("non-empty viewport");
        assert_eq!(vp.physical_size, UVec2::new(1280, 720));
    }

    // When the panel fills the window the free area collapses → no viewport, so
    // the camera falls back to the full window instead of a zero-size scissor.
    #[test]
    fn collapsed_area_yields_none() {
        assert!(world_viewport(Vec2::new(0.0, 960.0), Vec2::ZERO, 1.0, UVec2::new(1280, 960)).is_none());
    }
}
