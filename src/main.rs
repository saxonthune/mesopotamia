use bevy::prelude::*;
use bevy_egui::{EguiPlugin};

mod field;
mod grid;
mod elk;
mod render;
mod ui;
mod river;

use crate::grid::GridPlugin;
use crate::elk::ElkPlugin;
use crate::ui::UiPlugin;
use render::RenderPlugin;
use crate::river::RiverPlugin;

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
            ElkPlugin,
            RiverPlugin,
        ))
        .run();
}
