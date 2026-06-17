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
#[path = "../elk/mod.rs"]
mod elk;
#[path = "../render.rs"]
mod render;
#[path = "../settings.rs"]
mod settings;
#[path = "../history.rs"]
mod history;
#[path = "../ui.rs"]
mod ui;
#[path = "../river/mod.rs"]
mod river;

use crate::elk::ElkSimPlugin;
use crate::grid::GridPlugin;
use crate::river::RiverPlugin;
use crate::settings::UserSettings;
use crate::ui::UiPlugin;
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
        .add_plugins((RenderPlugin, UiPlugin, GridPlugin, ElkSimPlugin, RiverPlugin))
        .insert_resource(settings);

    // Maximizing is native-only; the canvas sizes the window on the web.
    #[cfg(not(target_arch = "wasm32"))]
    app.add_systems(Startup, settings::maximize_window);

    app.run();
}
