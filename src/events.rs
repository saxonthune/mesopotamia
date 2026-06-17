use bevy::prelude::*;
use std::collections::VecDeque;

// Match history.rs's WINDOW so the ring-buffer holds the same simulation span.
const EVENT_CAP: usize = 6000;

/// Extensible enumeration of observable simulation events.
/// Only `Starved` is emitted now; new variants extend this without touching
/// the ring-buffer or the existing UI.
#[derive(Clone, Copy, Debug)]
pub enum EventKind {
    Starved,
}

/// A single observable event captured from the simulation.
#[derive(Clone)]
pub struct Event {
    pub tick: u64,
    pub cell: usize,
    pub kind: EventKind,
    pub energy: f32,
    /// Chosen movement direction from the elk's last decision, when present.
    pub chosen_step: Option<(isize, isize)>,
}

/// Ring-buffer of recent simulation events, bounded to `EVENT_CAP`.
/// Observational only — nothing in the sim reads or branches on this resource.
#[derive(Resource, Default)]
pub struct EventLog {
    pub recent: VecDeque<Event>,
}

impl EventLog {
    pub fn push(&mut self, event: Event) {
        if self.recent.len() >= EVENT_CAP {
            self.recent.pop_front();
        }
        self.recent.push_back(event);
    }
}

pub struct EventsPlugin;

impl Plugin for EventsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EventLog>();
    }
}
