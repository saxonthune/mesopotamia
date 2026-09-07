use bevy::prelude::*;
use std::collections::VecDeque;

// Match metrics.rs's LIVE_WINDOW so the ring-buffer holds the same simulation span.
const EVENT_CAP: usize = 6000;

#[derive(Clone, Copy, Debug)]
pub enum EventKind {
    Starved,
    Departed,
}

#[derive(Clone)]
pub struct Event {
    pub tick: u64,
    pub cell: usize,
    pub kind: EventKind,
    pub energy: f32,
    pub chosen_step: Option<(isize, isize)>,
}

// Observational only — nothing in the sim reads or branches on this.
#[derive(Resource, Default)]
pub struct EventLog {
    pub recent: VecDeque<Event>,
    // Monotonic total; score system advances against this so each despawn is folded in exactly once.
    pub total: u64,
}

impl EventLog {
    pub fn push(&mut self, event: Event) {
        if self.recent.len() >= EVENT_CAP {
            self.recent.pop_front();
        }
        self.recent.push_back(event);
        self.total += 1;
    }
}

pub struct EventsPlugin;

impl Plugin for EventsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EventLog>();
    }
}
