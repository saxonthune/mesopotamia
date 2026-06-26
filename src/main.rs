use bevy::prelude::*;
use bevy::window::WindowResolution;
use bevy_egui::EguiPlugin;

use mesopotamia::grid::GridPlugin;
use mesopotamia::death_marker::DeathMarkerPlugin;
use mesopotamia::elk::ElkSimPlugin;
use mesopotamia::overlay::OverlayPlugin;
use mesopotamia::settings::{self, UserSettings};
use mesopotamia::sim::SimStatePlugin;
use mesopotamia::ui::UiPlugin;
use mesopotamia::unit_select::UnitSelectPlugin;
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
        // DroppingsPlugin intentionally omitted; import and add it here to re-enable.
        .add_plugins((RenderPlugin, UiPlugin, UnitSelectPlugin, GridPlugin, ElkSimPlugin, WorldgenPlugin, SimStatePlugin, OverlayPlugin, DeathMarkerPlugin))
        .insert_resource(settings);

    #[cfg(not(target_arch = "wasm32"))]
    app.add_systems(Startup, settings::maximize_window);

    app.run();
}
