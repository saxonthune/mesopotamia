use bevy::app::ScheduleRunnerPlugin;
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use std::time::Duration;

use crate::elk::{DriveSamples, Elk, ElkParams, ElkSimPlugin, Herds};
use crate::grid::{Grid, GridPlugin};
use crate::river::RiverPlugin;

const HZ: f64 = 10.0;
const PERIOD: Duration = Duration::from_millis(100);

/// Build a headless app without a renderer or window.
pub fn make_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins.set(ScheduleRunnerPlugin::run_once()))
        .insert_resource(Time::<Fixed>::from_hz(HZ))
        .insert_resource(TimeUpdateStrategy::ManualDuration(PERIOD))
        .add_plugins((GridPlugin, ElkSimPlugin, RiverPlugin));
    app
}

pub fn elk_count(world: &mut World) -> usize {
    let mut q = world.query::<&Elk>();
    q.iter(world).count()
}

pub fn max_col_reached(world: &mut World) -> usize {
    let grid_width = world.get_resource::<Grid>().unwrap().width();
    let mut q = world.query::<&Elk>();
    q.iter(world).map(|elk| elk.cell % grid_width).max().unwrap_or(0)
}

/// Per-run outcome of a headless simulation, defined by doc02.02.
pub struct RunMetrics {
    pub survival: f32,
    pub max_col: usize,
    pub mean_migration_share: f32,
}

/// Run a headless app for `ticks` steps with the given `ElkParams` and collect
/// the doc02.02 metrics: survival, farthest column reached, and time-averaged
/// migration share across active pack slots.
pub fn run_metrics(params: ElkParams, ticks: u32) -> RunMetrics {
    let mut app = make_app();
    // Override the plugin default — insert_resource replaces init_resource's value.
    app.insert_resource(params);

    let mut share_acc = 0.0_f32;
    let mut share_ticks = 0u32;
    let mut max_col = 0usize;

    for _ in 0..ticks {
        app.update();
        let world = app.world_mut();

        // Sample per-slot migration share for this tick (borrow dropped before query below).
        let tick_share: f32 = {
            let samples = world.get_resource::<DriveSamples>().unwrap();
            let active: Vec<f32> = samples
                .per_slot
                .iter()
                .filter(|s| s.count > 0)
                .map(|s| s.migration_share())
                .collect();
            if active.is_empty() { 0.0 } else { active.iter().sum::<f32>() / active.len() as f32 }
        };
        share_acc += tick_share;
        share_ticks += 1;

        let col = max_col_reached(world);
        if col > max_col {
            max_col = col;
        }
    }

    let mean_migration_share =
        if share_ticks > 0 { share_acc / share_ticks as f32 } else { 0.0 };

    let world = app.world_mut();
    let herds = world.get_resource::<Herds>().unwrap();
    let (total_deaths, total_spawned) =
        herds.cohorts.values().fold((0u32, 0u32), |(d, s), c| {
            (d + c.deaths, s + c.alive + c.deaths + c.departures)
        });
    let survival = if total_spawned > 0 {
        1.0 - total_deaths as f32 / total_spawned as f32
    } else {
        1.0
    };

    RunMetrics { survival, max_col, mean_migration_share }
}

/// doc02.02 counterfactual: run with `migration = 0` and return the farthest
/// column the herd reached — natural drives alone carry the journey.
pub fn journey_natural(params: ElkParams, ticks: u32) -> usize {
    let mut p = params;
    p.migration = 0.0;
    run_metrics(p, ticks).max_col
}
