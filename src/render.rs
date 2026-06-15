use bevy::prelude::*;
use bevy::camera::{visibility::RenderLayers, CameraOutputMode};
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit};
use bevy::input::gestures::PinchGesture;
use bevy::ecs::schedule::common_conditions::not;
use bevy::render::render_resource::BlendState;
use bevy_egui::input::egui_wants_any_pointer_input;
use bevy_egui::{EguiGlobalSettings, PrimaryEguiContext};

use crate::elk::{Elk, elk_color};
use crate::grid::{Grid, GRID_HEIGHT, GRID_WIDTH, MAX_BROWSE, MAX_GRASS, MAX_POOP, MAX_WATER};

const TILE_SIZE: f32 = 16.0;

pub struct RenderPlugin;

impl Plugin for RenderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CameraSettings>()
            .add_systems(Startup, setup)
            .add_systems(
                Update,
                (
                    // World-view input is ignored while the pointer is over egui,
                    // so scrolling/zooming inside the panel doesn't also move the
                    // world behind it.
                    (scroll_input, pinch_zoom, pan).run_if(not(egui_wants_any_pointer_input)),
                    sync_tiles,
                    sync_poop,
                    sync_browse,
                    apply_camera,
                    sync_elk_transform,
                    sync_elk_color,
                ),
            );
    }
}

/// View state for the camera. Both the mouse (scroll/drag) and the UI sliders
/// write here; `apply_camera` is the single place that pushes it to the camera.
#[derive(Resource)]
pub struct CameraSettings {
    pub zoom: f32,
    pub pan: Vec2,
}

impl Default for CameraSettings {
    fn default() -> Self {
        Self { zoom: 1.0, pan: Vec2::ZERO }
    }
}

/// Marks the camera that renders the simulation world, as opposed to the egui
/// overlay camera. Systems that move or clip the world view query this so they
/// don't accidentally grab the UI camera.
#[derive(Component)]
pub struct WorldCamera;

#[derive(Component)]
struct CellTile {
    index: usize,
}

#[derive(Component)]
struct PoopDot {
    index: usize,
}

#[derive(Component)]
struct BrowseDot {
    index: usize,
}

pub fn cell_world_pos(grid: &Grid, index: usize) -> Vec2 {
    let (col, row) = grid.col_row(index);
    let x = (col as f32 - grid.width() as f32 / 2.0 + 0.5) * TILE_SIZE;
    let y = (row as f32 - grid.height() as f32 / 2.0 + 0.5) * TILE_SIZE;
    Vec2::new(x, y)
}

fn setup(
    mut commands: Commands,
    mut egui_settings: ResMut<EguiGlobalSettings>,
    grid: Res<Grid>,
) {
    // egui and the world need *separate* cameras. Confining the world camera's
    // viewport to the area above the dock must not also confine egui — if they
    // share a camera, shrinking the viewport shrinks the UI too and the two
    // feed back on each other. So we disable bevy_egui's auto primary context
    // and render egui through its own full-window camera composited on top.
    // (Pattern from the bevy_egui 0.39 `side_panel` example.)
    egui_settings.auto_create_primary_context = false;

    // World camera — `set_camera_viewport` clips this one to sit above the dock.
    commands.spawn((Camera2d, WorldCamera));

    // egui camera — full window, renders none of the world (RenderLayers::none),
    // draws after it (order 1), and alpha-composites the UI over the world
    // without clearing what the world camera drew.
    commands.spawn((
        PrimaryEguiContext,
        Camera2d,
        RenderLayers::none(),
        Camera {
            order: 1,
            output_mode: CameraOutputMode::Write {
                blend_state: Some(BlendState::ALPHA_BLENDING),
                clear_color: ClearColorConfig::None,
            },
            clear_color: ClearColorConfig::Custom(Color::NONE),
            ..default()
        },
    ));

    for index in 0..grid.len() {
        let pos = cell_world_pos(&grid, index);

        commands.spawn((
            Sprite::from_color(Color::srgb(0.3, 0.2, 0.1),
                Vec2::splat(TILE_SIZE - 1.0)),
            Transform::from_xyz(pos.x, pos.y, 0.0),
            CellTile { index }
        ));

        commands.spawn((
            Sprite::from_color(Color::srgb(0.25, 0.15, 0.05),
                Vec2::splat(TILE_SIZE * 0.3)),
            Transform::from_xyz(pos.x, pos.y, 0.5)
                .with_scale(Vec3::ZERO),
            PoopDot { index }
        ));

        // Browse shrub — a fat dark-green clump that grows in from zero scale, set
        // below the poop/elk layers so bodies read on top of it.
        commands.spawn((
            Sprite::from_color(Color::srgb(0.16, 0.30, 0.10),
                Vec2::splat(TILE_SIZE * 0.8)),
            Transform::from_xyz(pos.x, pos.y, 0.4)
                .with_scale(Vec3::ZERO),
            BrowseDot { index }
        ));
    }
}

/// Scroll input means different things per device. A mouse wheel reports in
/// `Line` units → zoom. A trackpad two-finger drag reports in `Pixel` units →
/// pan. Telling them apart by `unit` is what lets one event source do both.
fn scroll_input(scroll: Res<AccumulatedMouseScroll>, mut settings: ResMut<CameraSettings>) {
    if scroll.delta == Vec2::ZERO {
        return;
    }
    match scroll.unit {
        MouseScrollUnit::Line => {
            let factor = 1.0 - scroll.delta.y * 0.1;
            settings.zoom = (settings.zoom * factor).clamp(0.1, 10.0);
        }
        MouseScrollUnit::Pixel => {
            // Same scaling/sign logic as the drag-pan below: scale by zoom so the
            // world tracks the fingers, flip y because screen-y points down.
            let zoom = settings.zoom;
            settings.pan.x -= scroll.delta.x * zoom;
            settings.pan.y += scroll.delta.y * zoom;
        }
    }
}

/// Trackpad pinch (macOS): positive delta = zoom in, which is a *smaller*
/// orthographic scale.
fn pinch_zoom(mut pinch: MessageReader<PinchGesture>, mut settings: ResMut<CameraSettings>) {
    for ev in pinch.read() {
        let factor = 1.0 - ev.0 * 3.0;
        settings.zoom = (settings.zoom * factor).clamp(0.1, 10.0);
    }
}

fn pan(
    buttons: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    mut settings: ResMut<CameraSettings>,
) {
    if !(buttons.pressed(MouseButton::Middle) || buttons.pressed(MouseButton::Right)) {
        return;
    }
    // Scale by zoom so one screen pixel of drag moves a constant amount of
    // *screen* regardless of zoom. Screen-y points down, world-y points up,
    // hence the flipped sign on y.
    let delta = motion.delta;
    let zoom = settings.zoom;
    settings.pan.x -= delta.x * zoom;
    settings.pan.y += delta.y * zoom;
}

fn apply_camera(
    mut settings: ResMut<CameraSettings>,
    camera: Single<(&mut Transform, &mut Projection), With<WorldCamera>>,
) {
    // The camera's translation is the world point at the viewport centre, so
    // keeping it inside the map guarantees no map corner can be panned past the
    // centre — the world always covers the middle of the view.
    let half_w = GRID_WIDTH as f32 * TILE_SIZE / 2.0;
    let half_h = GRID_HEIGHT as f32 * TILE_SIZE / 2.0;
    settings.pan.x = settings.pan.x.clamp(-half_w, half_w);
    settings.pan.y = settings.pan.y.clamp(-half_h, half_h);

    let (mut transform, mut projection) = camera.into_inner();
    transform.translation.x = settings.pan.x;
    transform.translation.y = settings.pan.y;
    if let Projection::Orthographic(ortho) = projection.as_mut() {
        ortho.scale = settings.zoom;
    }
}

fn sync_tiles(grid: Res<Grid>, mut tiles: Query<(&CellTile, &mut Sprite)>) {
    for (tile, mut sprite) in &mut tiles {
        let h = grid.grass(tile.index) / MAX_GRASS;
        let dirt = Vec3::new(0.76, 0.68, 0.48); // tan bare ground
        let green = Vec3::new(0.2, 0.7, 0.3);
        let c = dirt.lerp(green, h);

        let w = grid.water(tile.index) / MAX_WATER;
        let water = Vec3::new(0.1, 0.3, 0.7);
        let c = c.lerp(water, w);

        sprite.color = Color::srgb(c.x, c.y, c.z);
    }
}

fn sync_poop(grid: Res<Grid>, mut dots: Query<(&PoopDot, &mut Transform)>) {
    for (dot, mut transform) in &mut dots {
        let p = grid.poop(dot.index) / MAX_POOP;
        transform.scale = Vec3::splat(p);
    }
}

fn sync_browse(grid: Res<Grid>, mut dots: Query<(&BrowseDot, &mut Transform)>) {
    for (dot, mut transform) in &mut dots {
        let b = grid.browse(dot.index) / MAX_BROWSE;
        transform.scale = Vec3::splat(b);
    }
}

/// World-space centre of a cell, derived from grid constants.
fn cell_pos(cell: usize) -> Vec2 {
    let col = (cell % GRID_WIDTH) as f32;
    let row = (cell / GRID_WIDTH) as f32;
    Vec2::new(
        (col - GRID_WIDTH as f32 / 2.0 + 0.5) * TILE_SIZE,
        (row - GRID_HEIGHT as f32 / 2.0 + 0.5) * TILE_SIZE,
    )
}

/// Slide each elk sprite from its previous cell to its current one across the tick.
fn sync_elk_transform(time: Res<Time<Fixed>>, mut elk: Query<(&Elk, &mut Transform)>) {
    let t = time.overstep_fraction();
    for (elk, mut transform) in &mut elk {
        let pos = cell_pos(elk.prev_cell).lerp(cell_pos(elk.cell), t);
        transform.translation.x = pos.x;
        transform.translation.y = pos.y;
    }
}

/// Recolour each elk sprite from the `grazing` flag the simulation sets.
fn sync_elk_color(mut elk: Query<(&Elk, &mut Sprite)>) {
    for (elk, mut sprite) in &mut elk {
        sprite.color = elk_color(elk.slot as usize, elk.grazing);
    }
}

