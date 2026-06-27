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
use crate::grass_tile::{rasterize_grass, GrassTileParams};
use crate::flower_tile::{rasterize_flower_patch, FlowerPatchParams};
use crate::shrub_tile::{rasterize_shrub, ShrubTileParams};
use crate::grid::{Grid, GRID_HEIGHT, GRID_WIDTH, MAX_SHRUBS, MAX_GRASS, MAX_POOP, MAX_ROUGH, MAX_WATER};

pub const TILE_SIZE: f32 = 16.0;

const TILE_INSET: f32 = 0.0;

pub struct RenderPlugin;

impl Plugin for RenderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CameraSettings>()
            .add_systems(Startup, setup)
            .add_systems(
                Update,
                (
                        (scroll_input, pinch_zoom, pan).run_if(not(egui_wants_any_pointer_input)),
                    release_buttons_on_focus_loss,
                    sync_tiles,
                    sync_poop,
                    sync_shrubs,
                    sync_flower_patches,
                    sync_grass_tiles,
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
        Self { zoom: 1.0, pan: Vec2::ZERO }
    }
}

/// Marks the world camera; distinguishes it from the egui overlay camera.
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
struct ShrubDot {
    index: usize,
}

#[derive(Component)]
struct GrassTile {
    index: usize,
}

const GRASS_LEVELS: usize = 5;
const GRASS_VARIANTS: usize = 4;

// Indexed [level-1][variant]; built once at startup, handles swapped per-tick.
#[derive(Resource)]
struct GrassPalette {
    tiles: Vec<Vec<Handle<Image>>>,
}

fn grass_image(images: &mut Assets<Image>, rho: f32, seed: u64) -> Handle<Image> {
    let p = GrassTileParams::default();
    let data = rasterize_grass(&p, rho, seed);
    let mut image = Image::new(
        Extent3d {
            width: p.canvas as u32,
            height: (p.canvas + p.overflow) as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::nearest();
    images.add(image)
}

fn build_grass_palette(images: &mut Assets<Image>) -> GrassPalette {
    let mut tiles = Vec::with_capacity(GRASS_LEVELS);
    for level in 1..=GRASS_LEVELS {
        let rho = level as f32 / GRASS_LEVELS as f32;
        let mut variants = Vec::with_capacity(GRASS_VARIANTS);
        for v in 0..GRASS_VARIANTS {
            // Mix level into the seed so each (level, variant) is its own layout.
            let seed = (level as u64) << 32 | v as u64;
            variants.push(grass_image(images, rho, seed));
        }
        tiles.push(variants);
    }
    GrassPalette { tiles }
}

fn grass_level(frac: f32) -> usize {
    let frac = frac.clamp(0.0, 1.0);
    if frac <= 1e-4 {
        return 0;
    }
    ((frac * GRASS_LEVELS as f32).ceil() as usize).clamp(1, GRASS_LEVELS)
}

const FLOWER_LEVELS: usize = 5;
const FLOWER_VARIANTS: usize = 4;

// Indexed [level][variant], level = river-distance colour band.
#[derive(Resource)]
struct FlowerPalette {
    tiles: Vec<Vec<Handle<Image>>>,
}

fn vec3_to_rgb8(c: Vec3) -> [u8; 3] {
    let ch = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    [ch(c.x), ch(c.y), ch(c.z)]
}

fn flower_image(images: &mut Assets<Image>, color: [u8; 3], seed: u64) -> Handle<Image> {
    let p = FlowerPatchParams::default();
    let data = rasterize_flower_patch(&p, color, seed);
    let mut image = Image::new(
        Extent3d {
            width: p.canvas as u32,
            height: p.canvas as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::nearest();
    images.add(image)
}

fn build_flower_palette(images: &mut Assets<Image>) -> FlowerPalette {
    let mut tiles = Vec::with_capacity(FLOWER_LEVELS);
    for level in 0..FLOWER_LEVELS {
        let river_dist = level as f32 / (FLOWER_LEVELS - 1) as f32;
        let color = vec3_to_rgb8(flower_color(river_dist));
        let mut variants = Vec::with_capacity(FLOWER_VARIANTS);
        for v in 0..FLOWER_VARIANTS {
            // Distinct seed space from the grass palette.
            let seed = ((level as u64) << 32 | v as u64) ^ 0xF10_0000;
            variants.push(flower_image(images, color, seed));
        }
        tiles.push(variants);
    }
    FlowerPalette { tiles }
}

fn flower_level(river_dist: f32) -> usize {
    let rd = river_dist.clamp(0.0, 1.0);
    (rd * (FLOWER_LEVELS - 1) as f32).round() as usize
}

const SHRUB_GREEN: [u8; 3] = [32, 73, 26];
const SHRUB_OLIVE: [u8; 3] = [85, 96, 26];
const SHRUB_VARIANTS: usize = 4;

struct ShrubPalette {
    west: Vec<Handle<Image>>, // riparian: green fill, olive rims
    east: Vec<Handle<Image>>, // steppe: olive fill, green rims
}

// lean_left mirrors trunk grain so the two sides of the divide lean opposite ways.
fn shrub_image(images: &mut Assets<Image>, fill: [u8; 3], detail: [u8; 3], seed: u64, lean_left: bool) -> Handle<Image> {
    let p = ShrubTileParams { lean_left, ..ShrubTileParams::default() };
    let data = rasterize_shrub(&p, fill, detail, seed);
    let mut image = Image::new(
        Extent3d {
            width: p.canvas as u32,
            height: p.canvas as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::nearest();
    images.add(image)
}

fn build_shrub_palette(images: &mut Assets<Image>) -> ShrubPalette {
    let mut west = Vec::with_capacity(SHRUB_VARIANTS);
    let mut east = Vec::with_capacity(SHRUB_VARIANTS);
    for v in 0..SHRUB_VARIANTS {
        let seed = (v as u64) ^ 0x5_8B00;
        west.push(shrub_image(images, SHRUB_GREEN, SHRUB_OLIVE, seed, false));
        east.push(shrub_image(images, SHRUB_OLIVE, SHRUB_GREEN, seed ^ 0xE57, true));
    }
    ShrubPalette { west, east }
}

#[derive(Component)]
struct FlowerPatch {
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
    mut images: ResMut<Assets<Image>>,
    grid: Res<Grid>,
) {
    let palette = build_grass_palette(&mut images);
    let grass_start = palette.tiles[0][0].clone();
    commands.insert_resource(palette);
    // Grass sprites are taller than TILE_SIZE: overflow rows spill above; shift y up by half the overflow.
    let grass_p = GrassTileParams::default();
    let grass_h = (TILE_SIZE - TILE_INSET) * (grass_p.canvas + grass_p.overflow) as f32
        / grass_p.canvas as f32;
    let grass_size = Vec2::new(TILE_SIZE - TILE_INSET, grass_h);
    let grass_y_off = (grass_h - (TILE_SIZE - TILE_INSET)) / 2.0;
    let flowers = build_flower_palette(&mut images);
    let flower_start = flowers.tiles[0][0].clone();
    commands.insert_resource(flowers);
    let shrubs = build_shrub_palette(&mut images);
    // Separate cameras: world camera fills the window, egui camera composites the UI on top.
    // Every UI surface is a screen-space overlay, so neither camera's viewport reflows with the panels.
    egui_settings.auto_create_primary_context = false;

    commands.spawn((Camera2d, WorldCamera));

    // egui camera: full window, order 1, alpha-composites over the world camera.
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
                Vec2::splat(TILE_SIZE - TILE_INSET)),
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

        let shrub_variant = index % SHRUB_VARIANTS;
        let shrub_side = if grid.east_of_river(index) { &shrubs.east } else { &shrubs.west };
        commands.spawn((
            Sprite {
                image: shrub_side[shrub_variant].clone(),
                custom_size: Some(Vec2::splat(TILE_SIZE * 0.8)),
                ..default()
            },
            Transform::from_xyz(pos.x, pos.y, 0.4)
                .with_scale(Vec3::ZERO),
            ShrubDot { index }
        ));

        commands.spawn((
            Sprite {
                image: grass_start.clone(),
                custom_size: Some(grass_size),
                ..default()
            },
            Transform::from_xyz(pos.x, pos.y + grass_y_off, 0.3)
                .with_scale(Vec3::ZERO),
            GrassTile { index }
        ));

        commands.spawn((
            Sprite {
                image: flower_start.clone(),
                custom_size: Some(Vec2::splat(TILE_SIZE - TILE_INSET)),
                ..default()
            },
            Transform::from_xyz(pos.x, pos.y, 0.7)
                .with_scale(Vec3::ZERO),
            FlowerPatch { index }
        ));
    }
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

fn sync_tiles(grid: Res<Grid>, mut tiles: Query<(&CellTile, &mut Sprite)>) {
    for (tile, mut sprite) in &mut tiles {
        let tan = Vec3::new(0.80, 0.72, 0.52);       // water_prox = 0, pale tan
        let mahogany = Vec3::new(0.40, 0.18, 0.12);  // muted reddish-brown
        let dry = match grid.soil_tint(tile.index) {
            1 => tan.lerp(mahogany, 0.09),
            2 => tan.lerp(mahogany, 0.18),
            _ => tan,
        };
        let hydrated = Vec3::new(0.40, 0.30, 0.18);  // water_prox = 1, dark brown
        // water_prox is 0 on flooded cells (they ARE water), so force moist=1.0 to keep the river bed dark.
        let moist = if grid.water(tile.index) > 0.0 { 1.0 } else { grid.water_prox(tile.index) };
        let c = dry.lerp(hydrated, moist);

        let r = grid.rough(tile.index) / MAX_ROUGH;
        let rough = Vec3::new(0.42, 0.38, 0.33); // dull grey-brown
        let c = c.lerp(rough, r);

        let c = flood_color(c, grid.water(tile.index) / MAX_WATER);

        sprite.color = Color::srgb(c.x, c.y, c.z);
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

fn sync_poop(grid: Res<Grid>, mut dots: Query<(&PoopDot, &mut Transform)>) {
    for (dot, mut transform) in &mut dots {
        let p = grid.poop(dot.index) / MAX_POOP;
        transform.scale = Vec3::splat(p);
    }
}

fn sync_shrubs(grid: Res<Grid>, mut dots: Query<(&ShrubDot, &mut Transform)>) {
    for (dot, mut transform) in &mut dots {
        let s = grid.shrubs(dot.index) / MAX_SHRUBS;
        transform.scale = Vec3::splat(s);
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

fn sync_flower_patches(
    grid: Res<Grid>,
    palette: Res<FlowerPalette>,
    mut patches: Query<(&FlowerPatch, &mut Transform, &mut Sprite)>,
) {
    for (patch, mut transform, mut sprite) in &mut patches {
        if !flower_bloomed(&grid, patch.index) {
            transform.scale = Vec3::ZERO;
            continue;
        }
        transform.scale = Vec3::ONE;
        let level = flower_level(grid.river_dist(patch.index));
        let variant = patch.index % FLOWER_VARIANTS;
        let handle = &palette.tiles[level][variant];
        if sprite.image != *handle {
            sprite.image = handle.clone();
        }
    }
}

fn sync_grass_tiles(
    grid: Res<Grid>,
    palette: Res<GrassPalette>,
    mut tiles: Query<(&GrassTile, &mut Transform, &mut Sprite)>,
) {
    for (tile, mut transform, mut sprite) in &mut tiles {
        let level = grass_level(grid.grass(tile.index) / MAX_GRASS);
        if level == 0 {
            transform.scale = Vec3::ZERO;
            continue;
        }
        transform.scale = Vec3::ONE;
        // Cell-stable variant so the layout doesn't flicker as grass grows.
        let variant = tile.index % GRASS_VARIANTS;
        let handle = &palette.tiles[level - 1][variant];
        if sprite.image != *handle {
            sprite.image = handle.clone();
        }
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

#[cfg(test)]
mod tests {
    use super::*;

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

