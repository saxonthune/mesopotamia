use bevy::prelude::*;

#[derive(States, Default, Clone, Eq, PartialEq, Hash, Debug)]
pub enum Sim {
    #[default]
    Generating,
    Running,
}

pub struct SimStatePlugin;

impl Plugin for SimStatePlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<Sim>();
    }
}
