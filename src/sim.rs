use bevy::prelude::*;

/// The lifecycle of the simulation. `Generating` runs world generation; `Running`
/// steps the sim. Regeneration (Phase 2) is a transition back to `Generating`.
#[derive(States, Default, Clone, Eq, PartialEq, Hash, Debug)]
pub enum Sim {
    /// Default at boot: world generation runs in `OnEnter(Sim::Generating)`.
    #[default]
    Generating,
    /// The sim steps: grid growth + elk systems run, gated on this state.
    Running,
}

/// Registers the `Sim` state. Added by every binary and the headless harness so
/// the same state path runs in app and test contexts.
pub struct SimStatePlugin;

impl Plugin for SimStatePlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<Sim>();
    }
}
