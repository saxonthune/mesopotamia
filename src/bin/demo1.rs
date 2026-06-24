//! Milestone 1 demo binary — the grazing/river simulation, packaged for the web.
//!
//! This shares the same simulation modules as `main.rs` via `#[path]` includes,
//! so logic tweaks land here too. What it owns separately is the entry point:
//! a window configured to bind to an HTML `<canvas>` so the same build runs both
//! natively (`cargo run --bin demo1`) and in the browser (wasm32 target).

use bevy::prelude::*;
use bevy::window::WindowResolution;
use bevy_egui::EguiPlugin;

#[path = "../field.rs"]
mod field;
#[path = "../grid.rs"]
mod grid;
#[path = "../grass_tile.rs"]
mod grass_tile;
#[path = "../flower_tile.rs"]
mod flower_tile;
#[path = "../shrub_tile.rs"]
mod shrub_tile;
// The droppings cycle compiles into this crate but is never constructed here
// (it's omitted from the plugin tuple below), so the module is dead code.
#[path = "../droppings.rs"]
#[allow(dead_code)]
mod droppings;
#[path = "../elk/mod.rs"]
mod elk;
#[path = "../death_marker.rs"]
mod death_marker;
#[path = "../events.rs"]
mod events;
#[path = "../metrics.rs"]
mod metrics;
#[path = "../render.rs"]
mod render;
#[path = "../overlay.rs"]
mod overlay;
#[path = "../settings.rs"]
mod settings;
#[path = "../history.rs"]
mod history;
#[path = "../sim.rs"]
mod sim;
#[path = "../ui.rs"]
mod ui;
#[path = "../unit_select.rs"]
mod unit_select;
#[path = "../river/mod.rs"]
mod river;
#[path = "../worldgen/mod.rs"]
mod worldgen;

use crate::death_marker::DeathMarkerPlugin;
use crate::elk::ElkSimPlugin;
use crate::grid::GridPlugin;
use crate::overlay::OverlayPlugin;
use crate::sim::SimStatePlugin;
use crate::worldgen::WorldgenPlugin;
use crate::settings::UserSettings;
use crate::ui::UiPlugin;
use crate::unit_select::UnitSelectPlugin;
use render::RenderPlugin;

/// CSS selector of the canvas the browser page provides. The per-demo HTML in
/// `web/` declares `<canvas id="game-canvas">`; Bevy renders into it.
const CANVAS_ID: &str = "#game-canvas";

fn main() {
    let settings = UserSettings::load();

    let mut app = App::new();
    app.insert_resource(Time::<Fixed>::from_hz(10.0))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Mesopotamia — Demo 1".into(),
                // Bind to the page's canvas on web; ignored on native.
                canvas: Some(CANVAS_ID.into()),
                // Track the canvas's CSS box so the sim fills whatever the page
                // sizes it to, instead of a fixed pixel resolution.
                fit_canvas_to_parent: true,
                // Let the browser keep its own keyboard shortcuts (F5, etc.).
                prevent_default_event_handling: false,
                // Native restore/initial size; ignored on web (canvas governs).
                resolution: WindowResolution::new(settings.window.width, settings.window.height),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(EguiPlugin::default())
        // The droppings nutrient cycle (poop → grass) is built and tested but
        // intentionally disabled in the shipped demo. To re-enable, add
        // `droppings::DroppingsPlugin` to this tuple.
        .add_plugins((RenderPlugin, UiPlugin, UnitSelectPlugin, GridPlugin, ElkSimPlugin, WorldgenPlugin, SimStatePlugin, OverlayPlugin, DeathMarkerPlugin))
        .insert_resource(settings);

    // Maximizing is native-only; the canvas sizes the window on the web.
    #[cfg(not(target_arch = "wasm32"))]
    app.add_systems(Startup, settings::maximize_window);

    app.run();
}
