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

/// Gap left around each tile sprite, in pixels: the tile is drawn at
/// `TILE_SIZE - TILE_INSET` so the camera's dark clear colour shows through the
/// seams as faint grid lines. 0.0 removes the outline entirely (tiles abut);
/// raise it for a more pronounced grid.
const TILE_INSET: f32 = 0.0;

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
struct ShrubDot {
    index: usize,
}

/// A cell's procedural grass tile — a transparent-background sprite of green
/// blades, baked by `grass_tile::rasterize_grass`. `sync_grass_tiles` swaps its
/// texture for the palette entry matching the cell's current grass fraction.
#[derive(Component)]
struct GrassTile {
    index: usize,
}

/// Number of non-empty grass-height levels baked into the palette. Level `k`
/// (1..=LEVELS) renders blades at ρ = k / LEVELS; below level 1 the tile is
/// hidden. More levels = finer height steps as grass grows and is grazed down.
const GRASS_LEVELS: usize = 5;
/// Seed-variants baked per level so neighbouring cells at the same height don't
/// share an identical blade layout (which would read as a repeating tile).
const GRASS_VARIANTS: usize = 4;

/// Baked grass textures, indexed `[level-1][variant]` for level in 1..=GRASS_LEVELS.
/// Built once at startup; `sync_grass_tiles` only ever swaps handles, never
/// re-rasterizes, so per-tick grass changes cost a handle compare, not a redraw.
#[derive(Resource)]
struct GrassPalette {
    tiles: Vec<Vec<Handle<Image>>>,
}

/// Rasterize one grass tile and register it as a nearest-sampled `Image` asset
/// so the blades stay crisp when the 32px canvas is drawn at tile size.
fn grass_image(images: &mut Assets<Image>, rho: f32, seed: u64) -> Handle<Image> {
    let p = GrassTileParams::default();
    let data = rasterize_grass(&p, rho, seed);
    let mut image = Image::new(
        Extent3d {
            width: p.canvas as u32,
            // Taller than wide: the tile rows plus the overflow band above.
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

/// Map a grass fraction in `[0, 1]` to a palette level: 0 = empty (hide the
/// tile), else 1..=GRASS_LEVELS over equal-width bands. Pure so the banding is
/// pinned by a test rather than read off the screen.
fn grass_level(frac: f32) -> usize {
    let frac = frac.clamp(0.0, 1.0);
    if frac <= 1e-4 {
        return 0;
    }
    ((frac * GRASS_LEVELS as f32).ceil() as usize).clamp(1, GRASS_LEVELS)
}

/// Number of colour bands the flower palette samples along the river-distance
/// ramp (white at the water → cyan on the divide).
const FLOWER_LEVELS: usize = 5;
/// Seed-variants baked per colour band so neighbouring patches differ in scatter.
const FLOWER_VARIANTS: usize = 4;

/// Baked flower-patch textures, indexed `[level][variant]`, level in 0..FLOWER_LEVELS
/// mapping the river-distance colour ramp. Built once at startup; the sync only
/// swaps handles.
#[derive(Resource)]
struct FlowerPalette {
    tiles: Vec<Vec<Handle<Image>>>,
}

/// Convert a linear `Vec3` colour in `[0, 1]` to RGB8 for the rasterizer.
fn vec3_to_rgb8(c: Vec3) -> [u8; 3] {
    let ch = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    [ch(c.x), ch(c.y), ch(c.z)]
}

/// Rasterize one flower patch and register it as a nearest-sampled `Image`.
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

/// Colour band for a cell's river distance: nearest of the `FLOWER_LEVELS` baked
/// samples. Pure so the banding is pinned by a test.
fn flower_level(river_dist: f32) -> usize {
    let rd = river_dist.clamp(0.0, 1.0);
    (rd * (FLOWER_LEVELS - 1) as f32).round() as usize
}

/// Riparian-green (west of a river) and steppe-olive (east) shrub tones. Each is
/// the fill for its own side and the complement detail for the other, so the two
/// tones serve as each other's bushel-rim pattern. The tints are pushed apart
/// from their midpoint so the continental divide reads as a clear step.
const SHRUB_GREEN: [u8; 3] = [32, 73, 26]; // 0.125, 0.285, 0.10
const SHRUB_OLIVE: [u8; 3] = [85, 96, 26]; // 0.335, 0.375, 0.10
/// Seed-variants per side so neighbouring shrub tiles don't share a bushel layout.
const SHRUB_VARIANTS: usize = 4;

/// Baked shrub textures, one set of variants per side of the divide. A cell's
/// shrub tile is fixed at spawn (its side and a stable variant), so the runtime
/// only scales it as the clump grows.
struct ShrubPalette {
    west: Vec<Handle<Image>>, // riparian: green fill, olive rims
    east: Vec<Handle<Image>>, // steppe: olive fill, green rims
}

/// Rasterize one shrub tile and register it as a nearest-sampled `Image`. `lean_left`
/// mirrors the trunk grain so the two sides of the divide read as opposite leans.
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
        // West (tone A) leans up-and-right; east (tone B) leans up-and-left.
        west.push(shrub_image(images, SHRUB_GREEN, SHRUB_OLIVE, seed, false));
        east.push(shrub_image(images, SHRUB_OLIVE, SHRUB_GREEN, seed ^ 0xE57, true));
    }
    ShrubPalette { west, east }
}

/// A cell's procedural flower patch — a transparent-background sprite of small
/// blossoms baked by `flower_tile::rasterize_flower_patch`, coloured only by the
/// cell's river-distance ramp. Scales in from zero when the shrub is fully grown;
/// `sync_flower_patches` picks the palette entry for the cell's colour band.
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
    // Bake the grass-tile palette once; every cell's grass sprite starts on the
    // level-1 variant-0 handle and `sync_grass_tiles` swaps from there.
    let palette = build_grass_palette(&mut images);
    let grass_start = palette.tiles[0][0].clone();
    commands.insert_resource(palette);
    // Grass sprites are taller than a tile: the bottom `canvas` rows cover the cell
    // and the `overflow` rows above spill onto the upper neighbour. Size the sprite
    // to match the raster's aspect and shift it up by half the overflow so the tile
    // region still lands exactly on the cell square.
    let grass_p = GrassTileParams::default();
    let grass_h = (TILE_SIZE - TILE_INSET) * (grass_p.canvas + grass_p.overflow) as f32
        / grass_p.canvas as f32;
    let grass_size = Vec2::new(TILE_SIZE - TILE_INSET, grass_h);
    let grass_y_off = (grass_h - (TILE_SIZE - TILE_INSET)) / 2.0;
    // Bake the flower-patch palette (one colour band per river-distance sample).
    let flowers = build_flower_palette(&mut images);
    let flower_start = flowers.tiles[0][0].clone();
    commands.insert_resource(flowers);
    // Bake the shrub palette (a set of bushel-layout variants per side of the divide).
    let shrubs = build_shrub_palette(&mut images);
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

        // Shrub — a procedural clump of bushels (a filled box overlaid with fuzzy
        // complement-rimmed circles) that grows in from zero scale, set below the
        // poop/elk layers so bodies read on top of it. Cells east of their nearest
        // river fill with the steppe olive (green rims); west cells fill green
        // (olive rims), so the invisible continental divide reads as a tone step.
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

        // Procedural grass tile — green blades over the dirt, sitting on the
        // ground below the shrub clump. Starts hidden (zero scale); the first
        // `sync_grass_tiles` pass reveals and textures it from the cell's grass.
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

        // Procedural flower patch — small blossoms over the grass, coloured only
        // by the cell's river-distance ramp (white→cyan). Sits above the grass so
        // the accent reads on top. Starts hidden; `sync_flower_patches` reveals it
        // on a fully-grown flowering shrub and picks the colour-band texture.
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

/// Scroll input means different things per device. Natively, a mouse wheel
/// reports in `Line` units → zoom, while a trackpad two-finger drag reports in
/// `Pixel` units → pan; telling them apart by `unit` lets one event source do
/// both. On the web that distinction is unreliable, so scroll always zooms (see
/// the wasm branch below).
fn scroll_input(scroll: Res<AccumulatedMouseScroll>, mut settings: ResMut<CameraSettings>) {
    if scroll.delta == Vec2::ZERO {
        return;
    }

    // On the web, browsers report wheel deltas inconsistently — whether a mouse
    // wheel arrives as `Line` or `Pixel` depends on the browser, not the device
    // — so the native pan/zoom split by unit is unreliable. Treat all scroll as
    // zoom there; pan stays on drag and the on-screen sliders. The two unit
    // scales differ only so a notch feels the same: `Line` is ~1 per notch,
    // `Pixel` ~100, hence the 0.01 normalization on the latter.
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

/// Belt-and-suspenders against a stuck pan. The right/middle-drag pan reads
/// `ButtonInput` each frame, so it relies on a matching button-up to stop. If
/// the window loses focus or the cursor leaves it mid-drag, that up can be
/// missed (the browser eats it), leaving the button "held" forever. Releasing
/// all mouse buttons on either event clears the phantom press.
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
        // The dirt tint rides `water_prox` — the same field that caps grass — so
        // a hydrated cell reads as dark, moist soil and the lime grass digit on
        // top tells one continuous story: wetter ground supports more grass.
        // Dry-soil base: plain tan, or a sparse speckle nudged toward mahogany —
        // a muted reddish-brown rather than vivid red, so the grain reads earthy.
        // Tint 1 is 9% of the way, tint 2 is 18% (keeping the 1:2 ratio).
        // Placement is authored by `seed_soil_texture`.
        let tan = Vec3::new(0.80, 0.72, 0.52);       // water_prox = 0, pale tan
        let mahogany = Vec3::new(0.40, 0.18, 0.12);  // muted reddish-brown
        let dry = match grid.soil_tint(tile.index) {
            1 => tan.lerp(mahogany, 0.09),
            2 => tan.lerp(mahogany, 0.18),
            _ => tan,
        };
        let hydrated = Vec3::new(0.40, 0.30, 0.18);  // water_prox = 1, dark brown
        // A flooded cell's own `water_prox` is 0 (it IS water — grass can't grow
        // there), so reading the tint straight off it would give the river bed a
        // bright dry-tan base. That base then bleeds through `flood_color`'s shallow
        // bank ramp, painting the river's shallow edge an odd light brown. Treat any
        // cell carrying water as fully hydrated so its soil base is wet dark soil and
        // the shallow edge reads muddy, matching the dark bank just outside it.
        let moist = if grid.water(tile.index) > 0.0 { 1.0 } else { grid.water_prox(tile.index) };
        let c = dry.lerp(hydrated, moist);

        // Rough terrain sits on top of soil: where present it pulls the tan
        // toward a dull grey-brown, scaled by intensity, so broken ground reads
        // distinctly against the pale-tan/dark-brown soil ramp.
        let r = grid.rough(tile.index) / MAX_ROUGH;
        let rough = Vec3::new(0.42, 0.38, 0.33); // dull grey-brown
        let c = c.lerp(rough, r);

        let c = flood_color(c, grid.water(tile.index) / MAX_WATER);

        sprite.color = Color::srgb(c.x, c.y, c.z);
    }
}

/// The water palette: a soft muddy bank at the land edge, then shades of blue from
/// a light shallow (fords/crossings) to a deep channel blue. The bank is kept close
/// to the hydrated-soil brown (`0.40, 0.30, 0.18`) so the land→water edge reads as a
/// gentle darkening rather than a harsh near-black ring around every river.
const WATER_BANK: Vec3 = Vec3::new(0.34, 0.26, 0.16); // muted muddy bank, edge only
// Shallow/ford water is the muddy bank lifted partway toward blue — a silty slate
// rather than a bright shallow — so the river edge against the bank reads as muddy
// water, not an oddly light strip. It is the shallow extrema; depth ramps it to deep.
const WATER_SHALLOW: Vec3 = Vec3::new(0.27, 0.37, 0.50); // silty muddy-blue — fords/edges
const WATER_DEEP: Vec3 = Vec3::new(0.07, 0.22, 0.55); // deep channel blue

/// Depth at/below which a flooding cell is still the dark bank; above it the cell
/// is blue. Kept small so the river body reads blue and only a hairline edge banks.
const WATER_BANK_EDGE: f32 = 0.12;

/// Blend land toward water as a cell floods. The depth range reads as blue —
/// silty/muddy at the shallows (so fords and river edges read as silty water rather
/// than a bright shallow) deepening to the channel blue — with the dark bank confined
/// to a hairline edge where water meets land. `water_frac` in `[0, 1]`. Pure so the
/// river contract is pinned by a test.
fn flood_color(land: Vec3, water_frac: f32) -> Vec3 {
    let w = water_frac.clamp(0.0, 1.0);
    // Shades of blue across the depth range: light shallow → deep channel.
    let blue = WATER_SHALLOW.lerp(WATER_DEEP, w);
    if w <= WATER_BANK_EDGE {
        // The very edge: land fades into the dark bank.
        land.lerp(WATER_BANK, w / WATER_BANK_EDGE)
    } else {
        // Just past the edge, rise out of the bank into blue over a short ramp so
        // the river body is blue rather than mud.
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

/// Fraction of its own capacity a shrub must reach before its flower blooms.
const FLOWER_BLOOM_FRAC: f32 = 0.95;

/// Whether a cell's flower is in bloom: it must bear a flower and its shrub must
/// have grown to (nearly) its full carrying capacity. Pure so the gate is one
/// place both flower sprites read.
fn flower_bloomed(grid: &Grid, index: usize) -> bool {
    let cap = grid.shrub_cap(index);
    grid.flower(index) && cap > 0.0 && grid.shrubs(index) >= FLOWER_BLOOM_FRAC * cap
}

/// Interpolate a flower's colour along its distance from the nearest river: a
/// straight ramp from white at the water to a softened cyan on the divide —
/// Alexander's repeating alternation. The ramp stops at 80% of the way to full
/// cyan so the divide reads as a gentle tint rather than saturated. Drives the
/// baked flower palette. Pure so the ramp is pinned by a test.
const FLOWER_CYAN_MAX: f32 = 0.8;

fn flower_color(river_dist: f32) -> Vec3 {
    let white = Vec3::ONE;
    let cyan = Vec3::new(0.0, 1.0, 1.0);
    white.lerp(cyan, river_dist.clamp(0.0, 1.0) * FLOWER_CYAN_MAX)
}

/// Reveal each cell's flower patch when its shrub is fully grown and pick the
/// palette texture for the cell's river-distance colour band and a cell-stable
/// variant. Only swaps the handle when the band changes (river distance is
/// static, so in practice once).
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

/// Texture each cell's grass tile from its current grass fraction: hide it (zero
/// scale) below level 1, else show the palette entry for the cell's height level
/// and a cell-stable variant. Only swaps the handle when the level changes, so a
/// static field costs a compare per cell and no redraw.
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
        // Dry land is untouched.
        assert_eq!(flood_color(land, 0.0), land);
        // The deep channel is the deep blue.
        let deep = flood_color(land, 1.0);
        assert!((deep - WATER_DEEP).length() < 1e-6, "deep water is the deep blue");
        // A ford / shallow crossing reads as blue (blue channel dominant), not brown.
        let ford = flood_color(land, 0.35);
        assert!(ford.z > ford.x && ford.z > ford.y, "ford is blue, blue dominant");
        // ...and as a lighter shade of blue than the deep channel.
        assert!(brightness(ford) > brightness(deep), "the ford is a lighter blue");
        // Only the hairline edge is the dark bank — darker than the river body.
        let bank = flood_color(land, 0.05);
        assert!(brightness(bank) < brightness(ford), "the bank edge is darker than the river");
    }

    #[test]
    fn flower_color_ramps_white_to_softened_cyan() {
        // At the water: pure white.
        assert_eq!(flower_color(0.0), Vec3::ONE);
        // On the divide: 80% of the way to cyan — red drops to 0.2, not 0.
        let divide = flower_color(1.0);
        assert!((divide.x - 0.2).abs() < 1e-6, "red softens to 0.2, not full cyan");
        assert!((divide.y - 1.0).abs() < 1e-6, "green stays full");
        assert!((divide.z - 1.0).abs() < 1e-6, "blue stays full");
        // Linear in between, no plateau: halfway is halfway up the ramp.
        let mid = flower_color(0.5);
        assert!((mid.x - 0.6).abs() < 1e-6, "midpoint is linear, red = 1 - 0.5*0.8");
    }
}

