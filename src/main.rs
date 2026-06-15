use bevy::prelude::*;
use bevy_egui::EguiPlugin;

use mesopotamia::grid::GridPlugin;
use mesopotamia::elk::ElkSimPlugin;
use mesopotamia::ui::UiPlugin;
use mesopotamia::render::RenderPlugin;
use mesopotamia::river::RiverPlugin;

fn main() {
    println!("Hello, world!");

    App::new()
        .insert_resource(Time::<Fixed>::from_hz(10.0))
        .add_plugins(DefaultPlugins)
        .add_plugins(EguiPlugin::default())
        .add_plugins((
            RenderPlugin,
            UiPlugin,
            GridPlugin,
            ElkSimPlugin,
            RiverPlugin,
        ))
        .run();
}
