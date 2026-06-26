//! Demo 1 binary: shares sim modules with `main.rs` via `#[path]` re-includes;
//! entry point binds to the page's `<canvas>` for web (`wasm32`) or runs natively.

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

const CANVAS_ID: &str = "#game-canvas"; // matches <canvas id="game-canvas"> in web/

fn main() {
    let settings = UserSettings::load();

    let mut app = App::new();
    app.insert_resource(Time::<Fixed>::from_hz(10.0))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Mesopotamia — Demo 1".into(),
                canvas: Some(CANVAS_ID.into()),
                fit_canvas_to_parent: true,
                prevent_default_event_handling: false,
                resolution: WindowResolution::new(settings.window.width, settings.window.height), // ignored on web

                ..default()
            }),
            ..default()
        }))
        .add_plugins(EguiPlugin::default())
        // DroppingsPlugin intentionally omitted; add it here to re-enable the nutrient cycle.
        .add_plugins((RenderPlugin, UiPlugin, UnitSelectPlugin, GridPlugin, ElkSimPlugin, WorldgenPlugin, SimStatePlugin, OverlayPlugin, DeathMarkerPlugin))
        .insert_resource(settings);

    #[cfg(not(target_arch = "wasm32"))]
    app.add_systems(Startup, settings::maximize_window);

    app.run();
}
