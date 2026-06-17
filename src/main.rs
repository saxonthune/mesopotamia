use bevy::prelude::*;
use bevy::window::WindowResolution;
use bevy_egui::EguiPlugin;

use mesopotamia::grid::GridPlugin;
use mesopotamia::elk::ElkSimPlugin;
use mesopotamia::settings::{self, UserSettings};
use mesopotamia::ui::UiPlugin;
use mesopotamia::render::RenderPlugin;
use mesopotamia::worldgen::WorldgenPlugin;

fn main() {
    let settings = UserSettings::load();

    let mut app = App::new();
    app.insert_resource(Time::<Fixed>::from_hz(10.0))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Mesopotamia".into(),
                resolution: WindowResolution::new(settings.window.width, settings.window.height),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(EguiPlugin::default())
        .add_plugins((RenderPlugin, UiPlugin, GridPlugin, ElkSimPlugin, WorldgenPlugin))
        .insert_resource(settings);

    // Maximizing is a runtime request and only meaningful natively; on the web
    // the canvas governs size.
    #[cfg(not(target_arch = "wasm32"))]
    app.add_systems(Startup, settings::maximize_window);

    app.run();
}
