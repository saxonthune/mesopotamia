use std::collections::VecDeque;

use bevy::prelude::*;

use crate::elk::{Elk, DriveSamples};

const WINDOW: usize = 600;

#[derive(Resource, Default)]
pub struct History {
    pub population: VecDeque<f32>,
    pub avg_energy: VecDeque<f32>,
    pub migration_share: Vec<VecDeque<f32>>, // per slot, from DriveSamples
}

fn push_capped(buf: &mut VecDeque<f32>, value: f32) {
    buf.push_back(value);
    if buf.len() > WINDOW {
        buf.pop_front();
    }
}

pub fn sample_history(
    elk: Query<&Elk>,
    drive_samples: Res<DriveSamples>,
    mut history: ResMut<History>,
) {
    // Ensure per-slot buffers are sized to match DriveSamples.
    let n_slots = drive_samples.per_slot.len();
    if history.migration_share.len() != n_slots {
        history.migration_share.resize_with(n_slots, VecDeque::new);
    }

    let mut total: u32 = 0;
    let mut energy_sum: f32 = 0.0;
    for e in &elk {
        total += 1;
        energy_sum += e.energy;
    }

    push_capped(&mut history.population, total as f32);
    let avg = if total > 0 { energy_sum / total as f32 } else { 0.0 };
    push_capped(&mut history.avg_energy, avg);

    for (slot, ds) in drive_samples.per_slot.iter().enumerate() {
        push_capped(&mut history.migration_share[slot], ds.migration_share());
    }
}
