use std::collections::VecDeque;

use bevy::prelude::*;

use crate::elk::abundance::{self, AbundanceParams};
use crate::elk::{Elk, ENERGY_DRAIN};
use crate::grid::{Grid, GrowthRate};

const WINDOW: usize = 6000;

#[derive(Resource, Default)]
pub struct History {
    pub population: VecDeque<f32>,
    pub avg_energy: VecDeque<f32>,
    pub grass_mass: VecDeque<f32>,           // Σ grass over the grid
    pub shrub_mass: VecDeque<f32>,           // Σ shrubs over the grid
    pub abundance_per_elk: VecDeque<f32>,    // slider-weighted nearby grass+energy ÷ herd size, herd-mean
    pub regrowth_drain_ratio: VecDeque<f32>, // nearby forage regrowth per elk ÷ drain, herd-mean (>1 ⇒ camp)
}

fn push_capped(buf: &mut VecDeque<f32>, value: f32) {
    buf.push_back(value);
    if buf.len() > WINDOW {
        buf.pop_front();
    }
}

pub fn sample_history(
    elk: Query<&Elk>,
    grid: Res<Grid>,
    growth: Res<GrowthRate>,
    ab_params: Res<AbundanceParams>,
    mut history: ResMut<History>,
) {
    let n_slots = crate::elk::PACK_COUNT;

    let energy_extract = crate::metrics::ELK_METRICS[0].extract;
    let mut total: u32 = 0;
    let mut energy_sum: f32 = 0.0;
    // Per-slot accumulators for the herd-level abundance metrics: centroid (col,
    // row sums), stored energy, and headcount.
    let mut sum_col = vec![0.0_f32; n_slots];
    let mut sum_row = vec![0.0_f32; n_slots];
    let mut energy_by_slot = vec![0.0_f32; n_slots];
    let mut count_by_slot = vec![0_u32; n_slots];
    for e in &elk {
        total += 1;
        energy_sum += energy_extract(e);
        let s = e.slot as usize;
        if s < n_slots {
            let (col, row) = grid.col_row(e.cell);
            sum_col[s] += col as f32;
            sum_row[s] += row as f32;
            energy_by_slot[s] += e.energy;
            count_by_slot[s] += 1;
        }
    }

    push_capped(&mut history.population, total as f32);
    let avg = if total > 0 { energy_sum / total as f32 } else { 0.0 };
    push_capped(&mut history.avg_energy, avg);

    push_capped(&mut history.grass_mass, grid.total_grass());
    push_capped(&mut history.shrub_mass, grid.total_shrubs());

    // Herd-level abundance: sample forage and regrowth around each live herd's
    // centroid, then average the per-capita values across herds. The
    // regrowth-÷-drain ratio is the proof metric — a herd-mean above 1 means the
    // patches refill faster than the herds eat, so migration never wins.
    let mut ab_sum = 0.0_f32;
    let mut ratio_sum = 0.0_f32;
    let mut herds = 0_u32;
    for s in 0..n_slots {
        let count = count_by_slot[s];
        if count == 0 {
            continue;
        }
        let col = (sum_col[s] / count as f32).round() as usize;
        let row = (sum_row[s] / count as f32).round() as usize;
        let grass_nearby = abundance::grass_in_radius(&grid, col, row, ab_params.radius);
        let regrowth_nearby =
            abundance::regrowth_in_radius(&grid, &growth, col, row, ab_params.radius);
        let abundance =
            abundance::local_abundance(grass_nearby, energy_by_slot[s], ab_params.energy_weight);
        ab_sum += abundance::per_capita(abundance, count);
        let regrowth_pe = abundance::per_capita(regrowth_nearby, count);
        ratio_sum += abundance::regrowth_drain_ratio(regrowth_pe, ENERGY_DRAIN);
        herds += 1;
    }
    let (ab_mean, ratio_mean) = if herds > 0 {
        (ab_sum / herds as f32, ratio_sum / herds as f32)
    } else {
        (0.0, 0.0)
    };
    push_capped(&mut history.abundance_per_elk, ab_mean);
    push_capped(&mut history.regrowth_drain_ratio, ratio_mean);
}
