use bevy::prelude::*;
use bevy::asset::RenderAssetUsages;
use bevy::image::{Image, ImageSampler};
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::camera::{visibility::RenderLayers, CameraOutputMode};
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit};
use bevy::input::gestures::PinchGesture;
use bevy::window::{CursorLeft, WindowFocused};
use bevy::ecs::schedule::common_conditions::not;
use bevy::render::render_resource::BlendState;
use bevy_egui::input::egui_wants_any_pointer_input;
use bevy_egui::{EguiGlobalSettings, PrimaryEguiContext};

use crate::elk::{Elk, elk_color};
use crate::grid::{Grid, GRID_HEIGHT, GRID_WIDTH, MAX_SHRUBS, MAX_GRASS, MAX_POOP, MAX_ROUGH, MAX_WATER};

pub const TILE_SIZE: f32 = 16.0;

#[derive(Resource, Default)]
pub struct PerfStats {
    pub visible_sprites: u32,
    pub hidden_sprites: u32,
    pub sync_runs: u32,
    sync_counter: u32,
    last_reset: f64,
}

pub struct RenderPlugin;

impl Plugin for RenderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CameraSettings>()
            .init_resource::<PerfStats>()
            .add_systems(Startup, setup)
            .add_systems(
                Update,
                (
                        (scroll_input, pinch_zoom, pan).run_if(not(egui_wants_any_pointer_input)),
                    release_buttons_on_focus_loss,
                    (sync_tiles, sync_overlays, count_sync_run)
                        .run_if(resource_changed::<Grid>),
                    gather_perf_stats,
                    apply_camera,
                    sync_elk_transform,
                    sync_elk_color,
                ),
            );
    }
}

// zoom + pan target; `apply_camera` is the sole writer.
#[derive(Resource)]
pub struct CameraSettings {
    pub zoom: f32,
    pub pan: Vec2,
}

impl Default for CameraSettings {
    fn default() -> Self {
        Self { zoom: 2.5, pan: Vec2::ZERO }
    }
}

/// Marks the world camera; distinguishes it from the egui overlay camera.
#[derive(Component)]
pub struct WorldCamera;

#[derive(Resource)]
struct TerrainTexture(Handle<Image>);

#[derive(Resource)]
struct OverlayTextures {
    poop: Handle<Image>,
    shrub: Handle<Image>,
    grass: Handle<Image>,
    flower: Handle<Image>,
}

const GRASS_LEVELS: usize = 5;

fn grass_level(frac: f32) -> usize {
    let frac = frac.clamp(0.0, 1.0);
    if frac <= 1e-4 {
        return 0;
    }
    ((frac * GRASS_LEVELS as f32).ceil() as usize).clamp(1, GRASS_LEVELS)
}

const SHRUB_GREEN: [u8; 3] = [32, 73, 26];
const SHRUB_OLIVE: [u8; 3] = [85, 96, 26];

pub fn cell_world_pos(grid: &Grid, index: usize) -> Vec2 {
    let (col, row) = grid.col_row(index);
    let x = (col as f32 - grid.width() as f32 / 2.0 + 0.5) * TILE_SIZE;
    let y = (row as f32 - grid.height() as f32 / 2.0 + 0.5) * TILE_SIZE;
    Vec2::new(x, y)
}

fn make_overlay(images: &mut Assets<Image>, w: usize, h: usize) -> Handle<Image> {
    let mut img = Image::new(
        Extent3d { width: w as u32, height: h as u32, depth_or_array_layers: 1 },
        TextureDimension::D2,
        vec![0u8; w * h * 4],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    img.sampler = ImageSampler::nearest();
    images.add(img)
}

fn setup(
    mut commands: Commands,
    mut egui_settings: ResMut<EguiGlobalSettings>,
    mut images: ResMut<Assets<Image>>,
    grid: Res<Grid>,
) {
    egui_settings.auto_create_primary_context = false;

    commands.spawn((Camera2d, WorldCamera));

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

    let w = grid.width();
    let h = grid.height();
    let full_size = Vec2::new(w as f32 * TILE_SIZE, h as f32 * TILE_SIZE);

    let mut terrain_image = Image::new(
        Extent3d { width: w as u32, height: h as u32, depth_or_array_layers: 1 },
        TextureDimension::D2,
        vec![255u8; w * h * 4],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    terrain_image.sampler = ImageSampler::nearest();
    let terrain_handle = images.add(terrain_image);
    commands.insert_resource(TerrainTexture(terrain_handle.clone()));

    commands.spawn((
        Sprite { image: terrain_handle, custom_size: Some(full_size), ..default() },
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));

    let grass_h = make_overlay(&mut images, w, h);
    let shrub_h = make_overlay(&mut images, w, h);
    let poop_h = make_overlay(&mut images, w, h);
    let flower_h = make_overlay(&mut images, w, h);

    commands.spawn((
        Sprite { image: grass_h.clone(), custom_size: Some(full_size), ..default() },
        Transform::from_xyz(0.0, 0.0, 0.3),
    ));
    commands.spawn((
        Sprite { image: shrub_h.clone(), custom_size: Some(full_size), ..default() },
        Transform::from_xyz(0.0, 0.0, 0.4),
    ));
    commands.spawn((
        Sprite { image: poop_h.clone(), custom_size: Some(full_size), ..default() },
        Transform::from_xyz(0.0, 0.0, 0.5),
    ));
    commands.spawn((
        Sprite { image: flower_h.clone(), custom_size: Some(full_size), ..default() },
        Transform::from_xyz(0.0, 0.0, 0.7),
    ));

    commands.insert_resource(OverlayTextures {
        poop: poop_h,
        shrub: shrub_h,
        grass: grass_h,
        flower: flower_h,
    });
}

// Mouse wheel (Line units) → zoom; trackpad two-finger (Pixel units) → pan. Web: always zoom.
fn scroll_input(scroll: Res<AccumulatedMouseScroll>, mut settings: ResMut<CameraSettings>) {
    if scroll.delta == Vec2::ZERO {
        return;
    }

    // Web: Line/Pixel units are unreliable per-browser; 0.01 normalises Pixel (~100/notch) to Line (~1/notch).
    #[cfg(target_arch = "wasm32")]
    {
        let step = match scroll.unit {
            MouseScrollUnit::Line => scroll.delta.y,
            MouseScrollUnit::Pixel => scroll.delta.y * 0.01,
        };
        let factor = 1.0 - step * 0.1;
        settings.zoom = (settings.zoom * factor).clamp(0.1, 10.0);
    }

    #[cfg(not(target_arch = "wasm32"))]
    match scroll.unit {
        MouseScrollUnit::Line => {
            let factor = 1.0 - scroll.delta.y * 0.1;
            settings.zoom = (settings.zoom * factor).clamp(0.1, 10.0);
        }
        MouseScrollUnit::Pixel => {
            let zoom = settings.zoom;
            settings.pan.x -= scroll.delta.x * zoom;
            settings.pan.y += scroll.delta.y * zoom;
        }
    }
}

// Positive pinch delta = zoom in = smaller ortho scale.
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
    // Flip y: screen-y points down, world-y points up.
    let delta = motion.delta;
    let zoom = settings.zoom;
    settings.pan.x -= delta.x * zoom;
    settings.pan.y += delta.y * zoom;
}

// Clears phantom held buttons when focus is lost mid-drag (browser eats the button-up).
fn release_buttons_on_focus_loss(
    mut focus: MessageReader<WindowFocused>,
    mut cursor_left: MessageReader<CursorLeft>,
    mut buttons: ResMut<ButtonInput<MouseButton>>,
) {
    let lost_focus = focus.read().any(|e| !e.focused);
    let cursor_left = cursor_left.read().count() > 0;
    if lost_focus || cursor_left {
        buttons.release_all();
    }
}

fn apply_camera(
    mut settings: ResMut<CameraSettings>,
    camera: Single<(&mut Transform, &mut Projection), With<WorldCamera>>,
) {
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

fn sync_tiles(grid: Res<Grid>, terrain: Res<TerrainTexture>, mut images: ResMut<Assets<Image>>) {
    let Some(image) = images.get_mut(&terrain.0) else { return };
    let Some(data) = image.data.as_mut() else { return };
    let width = grid.width();
    let height = grid.height();
    for index in 0..grid.len() {
        let (col, row) = grid.col_row(index);
        let tan = Vec3::new(0.80, 0.72, 0.52);
        let mahogany = Vec3::new(0.40, 0.18, 0.12);
        let dry = match grid.soil_tint(index) {
            1 => tan.lerp(mahogany, 0.09),
            2 => tan.lerp(mahogany, 0.18),
            _ => tan,
        };
        let hydrated = Vec3::new(0.40, 0.30, 0.18);
        let moist = if grid.water(index) > 0.0 { 1.0 } else { grid.water_prox(index) };
        let c = dry.lerp(hydrated, moist);

        let r = grid.rough(index) / MAX_ROUGH;
        let rough = Vec3::new(0.42, 0.38, 0.33);
        let c = c.lerp(rough, r);

        let c = flood_color(c, grid.water(index) / MAX_WATER);

        let tex_row = height - 1 - row;
        let pixel = (tex_row * width + col) * 4;
        data[pixel] = (c.x.clamp(0.0, 1.0) * 255.0) as u8;
        data[pixel + 1] = (c.y.clamp(0.0, 1.0) * 255.0) as u8;
        data[pixel + 2] = (c.z.clamp(0.0, 1.0) * 255.0) as u8;
        data[pixel + 3] = 255;
    }
}

const WATER_BANK: Vec3 = Vec3::new(0.34, 0.26, 0.16);    // muddy edge
const WATER_SHALLOW: Vec3 = Vec3::new(0.27, 0.37, 0.50); // silty blue — fords
const WATER_DEEP: Vec3 = Vec3::new(0.07, 0.22, 0.55);    // deep channel

// Below this depth the cell shows as dark bank; above it the cell is blue.
const WATER_BANK_EDGE: f32 = 0.12;

fn flood_color(land: Vec3, water_frac: f32) -> Vec3 {
    let w = water_frac.clamp(0.0, 1.0);
    let blue = WATER_SHALLOW.lerp(WATER_DEEP, w);
    if w <= WATER_BANK_EDGE {
        land.lerp(WATER_BANK, w / WATER_BANK_EDGE)
    } else {
        // Short ramp out of the bank so the river body reads blue, not mud.
        let u = ((w - WATER_BANK_EDGE) / 0.12).clamp(0.0, 1.0);
        WATER_BANK.lerp(blue, u)
    }
}

const FLOWER_BLOOM_FRAC: f32 = 0.95;

fn flower_bloomed(grid: &Grid, index: usize) -> bool {
    let cap = grid.shrub_cap(index);
    grid.flower(index) && cap > 0.0 && grid.shrubs(index) >= FLOWER_BLOOM_FRAC * cap
}

const FLOWER_CYAN_MAX: f32 = 0.8;

fn flower_color(river_dist: f32) -> Vec3 {
    let white = Vec3::ONE;
    let cyan = Vec3::new(0.0, 1.0, 1.0);
    white.lerp(cyan, river_dist.clamp(0.0, 1.0) * FLOWER_CYAN_MAX)
}

const GRASS_BASE: Vec3 = Vec3::new(40.0 / 255.0, 95.0 / 255.0, 30.0 / 255.0);
const GRASS_TIP: Vec3 = Vec3::new(120.0 / 255.0, 180.0 / 255.0, 70.0 / 255.0);
const POOP_COLOR: [u8; 3] = [64, 38, 13];

fn sync_overlay(grid: &Grid, data: &mut [u8], writer: impl Fn(&Grid, usize) -> [u8; 4]) {
    let width = grid.width();
    let height = grid.height();
    for index in 0..grid.len() {
        let (col, row) = grid.col_row(index);
        let tex_row = height - 1 - row;
        let pixel = (tex_row * width + col) * 4;
        let rgba = writer(grid, index);
        data[pixel] = rgba[0];
        data[pixel + 1] = rgba[1];
        data[pixel + 2] = rgba[2];
        data[pixel + 3] = rgba[3];
    }
}

fn sync_overlays(
    grid: Res<Grid>,
    overlays: Res<OverlayTextures>,
    mut images: ResMut<Assets<Image>>,
) {
    if let Some(data) = images.get_mut(&overlays.poop).and_then(|i| i.data.as_mut()) {
        sync_overlay(&grid, data, |g, i| {
            let a = (g.poop(i) / MAX_POOP).clamp(0.0, 1.0);
            [POOP_COLOR[0], POOP_COLOR[1], POOP_COLOR[2], (a * 255.0) as u8]
        });
    }

    if let Some(data) = images.get_mut(&overlays.shrub).and_then(|i| i.data.as_mut()) {
        sync_overlay(&grid, data, |g, i| {
            let frac = (g.shrubs(i) / MAX_SHRUBS).clamp(0.0, 1.0);
            let fill = if g.east_of_river(i) { SHRUB_OLIVE } else { SHRUB_GREEN };
            [fill[0], fill[1], fill[2], (frac * 255.0) as u8]
        });
    }

    if let Some(data) = images.get_mut(&overlays.grass).and_then(|i| i.data.as_mut()) {
        sync_overlay(&grid, data, |g, i| {
            let level = grass_level(g.grass(i) / MAX_GRASS);
            if level == 0 {
                return [0, 0, 0, 0];
            }
            let t = level as f32 / GRASS_LEVELS as f32;
            let c = GRASS_BASE.lerp(GRASS_TIP, t);
            [
                (c.x.clamp(0.0, 1.0) * 255.0) as u8,
                (c.y.clamp(0.0, 1.0) * 255.0) as u8,
                (c.z.clamp(0.0, 1.0) * 255.0) as u8,
                (t * 0.7 * 255.0) as u8,
            ]
        });
    }

    if let Some(data) = images.get_mut(&overlays.flower).and_then(|i| i.data.as_mut()) {
        sync_overlay(&grid, data, |g, i| {
            if !flower_bloomed(g, i) {
                return [0, 0, 0, 0];
            }
            let c = flower_color(g.river_dist(i));
            [
                (c.x.clamp(0.0, 1.0) * 255.0) as u8,
                (c.y.clamp(0.0, 1.0) * 255.0) as u8,
                (c.z.clamp(0.0, 1.0) * 255.0) as u8,
                220,
            ]
        });
    }
}

// Lerps prev_cell→cell using the fixed-step overstep fraction for smooth render-rate motion.
fn sync_elk_transform(time: Res<Time<Fixed>>, grid: Res<Grid>, mut elk: Query<(&Elk, &mut Transform)>) {
    let over = time.overstep_fraction();
    for (elk, mut transform) in &mut elk {
        let t_start = (elk.move_t - elk.move_rate).max(0.0);
        let t = (t_start + elk.move_rate * over).clamp(0.0, 1.0);
        let p = cell_world_pos(&grid, elk.prev_cell).lerp(cell_world_pos(&grid, elk.cell), t);
        transform.translation.x = p.x;
        transform.translation.y = p.y;
    }
}

fn sync_elk_color(mut elk: Query<(&Elk, &mut Sprite)>) {
    for (elk, mut sprite) in &mut elk {
        sprite.color = elk_color(elk.slot as usize, elk.grazing);
    }
}

fn count_sync_run(mut stats: ResMut<PerfStats>) {
    stats.sync_counter += 1;
}

fn gather_perf_stats(
    time: Res<Time>,
    mut stats: ResMut<PerfStats>,
    sprites: Query<&Visibility, With<Sprite>>,
) {
    let mut visible = 0u32;
    let mut hidden = 0u32;
    for vis in &sprites {
        if *vis == Visibility::Hidden {
            hidden += 1;
        } else {
            visible += 1;
        }
    }
    stats.visible_sprites = visible;
    stats.hidden_sprites = hidden;

    let now = time.elapsed_secs_f64();
    if now - stats.last_reset >= 1.0 {
        stats.sync_runs = stats.sync_counter;
        stats.sync_counter = 0;
        stats.last_reset = now;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FLOWER_LEVELS: usize = 5;

    fn flower_level(river_dist: f32) -> usize {
        let rd = river_dist.clamp(0.0, 1.0);
        (rd * (FLOWER_LEVELS - 1) as f32).round() as usize
    }

    #[test]
    fn grass_level_bands() {
        assert_eq!(grass_level(0.0), 0);
        assert_eq!(grass_level(0.01), 1); // tiny positive lands in level 1
        assert_eq!(grass_level(0.2), 1);
        assert_eq!(grass_level(0.21), 2);
        assert_eq!(grass_level(0.6), 3);
        assert_eq!(grass_level(1.0), GRASS_LEVELS);
        assert_eq!(grass_level(2.0), GRASS_LEVELS, "clamps over 1.0");
    }

    #[test]
    fn flower_level_bands() {
        assert_eq!(flower_level(0.0), 0);
        assert_eq!(flower_level(1.0), FLOWER_LEVELS - 1);
        // Nearest sample: just over the first half-band rounds up to band 1.
        assert_eq!(flower_level(0.5 / (FLOWER_LEVELS - 1) as f32 + 0.01), 1);
        assert_eq!(flower_level(2.0), FLOWER_LEVELS - 1, "clamps over 1.0");
    }

    #[test]
    fn river_reads_blue_with_a_hairline_dark_bank() {
        let land = Vec3::new(0.5, 0.4, 0.25);
        let brightness = |c: Vec3| c.x + c.y + c.z;
        assert_eq!(flood_color(land, 0.0), land);
        let deep = flood_color(land, 1.0);
        assert!((deep - WATER_DEEP).length() < 1e-6, "deep water is the deep blue");
        let ford = flood_color(land, 0.35);
        assert!(ford.z > ford.x && ford.z > ford.y, "ford is blue, blue dominant");
        assert!(brightness(ford) > brightness(deep), "the ford is a lighter blue");
        let bank = flood_color(land, 0.05);
        assert!(brightness(bank) < brightness(ford), "the bank edge is darker than the river");
    }

    #[test]
    fn flower_color_ramps_white_to_softened_cyan() {
        assert_eq!(flower_color(0.0), Vec3::ONE);
        let divide = flower_color(1.0);
        assert!((divide.x - 0.2).abs() < 1e-6, "red softens to 0.2, not full cyan");
        assert!((divide.y - 1.0).abs() < 1e-6, "green stays full");
        assert!((divide.z - 1.0).abs() < 1e-6, "blue stays full");
        let mid = flower_color(0.5);
        assert!((mid.x - 0.6).abs() < 1e-6, "midpoint is linear, red = 1 - 0.5*0.8");
    }
}

