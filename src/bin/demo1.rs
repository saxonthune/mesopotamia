//! Demo 1 binary: the sole web/native entry point. It assembles the game from the `mesopotamia`
//! library crate; on web it binds the page's `<canvas>`, and wasm-bindgen targets this binary.

use bevy::diagnostic::{EntityCountDiagnosticsPlugin, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use bevy::window::WindowResolution;
use bevy_egui::EguiPlugin;

use mesopotamia::elk::presets;
use mesopotamia::elk::{ElkParams, RatioControls};
use mesopotamia::death_marker::DeathMarkerPlugin;
use mesopotamia::elk::ElkSimPlugin;
use mesopotamia::grid::GridPlugin;
use mesopotamia::metrics::InstrumentPlugin;
use mesopotamia::overlay::OverlayPlugin;
use mesopotamia::render::RenderPlugin;
use mesopotamia::settings::UserSettings;
use mesopotamia::sim::SimStatePlugin;
use mesopotamia::ui::UiPlugin;
use mesopotamia::unit_select::UnitSelectPlugin;
use mesopotamia::worldgen::WorldgenPlugin;

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
        .add_plugins((FrameTimeDiagnosticsPlugin::default(), EntityCountDiagnosticsPlugin::default()))
        .add_plugins((RenderPlugin, UiPlugin, UnitSelectPlugin, GridPlugin, ElkSimPlugin, WorldgenPlugin, SimStatePlugin, OverlayPlugin, DeathMarkerPlugin, InstrumentPlugin::default()))
        .add_systems(Startup, apply_optimized_preset)
        .insert_resource(settings);

    #[cfg(not(target_arch = "wasm32"))]
    {
        use mesopotamia::grid::Grid;
        use mesopotamia::worldgen::testmap;

        app.add_systems(Startup, mesopotamia::settings::maximize_window);
        match testmap::source_from_args(std::env::args()) {
            Ok(source) => {
                if let testmap::WorldSource::TestMap(m) = source {
                    app.insert_resource(Grid::new(m.width, m.height));
                }
                app.insert_resource(source);
            }
            Err(msg) => {
                eprintln!("{msg}");
                std::process::exit(2);
            }
        }
    }

    app.run();
}

fn apply_optimized_preset(mut ratios: ResMut<RatioControls>, mut params: ResMut<ElkParams>) {
    presets::apply(&presets::PRESETS[2], &mut ratios, &mut params);
}
