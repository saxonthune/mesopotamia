use bevy::app::ScheduleRunnerPlugin;
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use mesopotamia::elk::{Elk, ElkSimPlugin};
use mesopotamia::grid::{Grid, GridPlugin};
use mesopotamia::river::RiverPlugin;
use std::time::Duration;

const HZ: f64 = 10.0;
const PERIOD: Duration = Duration::from_millis(100);

// Mirrors the private constants in elk.rs.
const TARGET_POPULATION: usize = 200;
const EDGE_COL: usize = 254; // GRID_WIDTH - 2

fn make_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins.set(ScheduleRunnerPlugin::run_once()))
        .insert_resource(Time::<Fixed>::from_hz(HZ))
        .insert_resource(TimeUpdateStrategy::ManualDuration(PERIOD))
        .add_plugins((GridPlugin, ElkSimPlugin, RiverPlugin));
    app
}

/// Build a headless app, step it `ticks` times, and return the final state.
// Used by later tasks that will import this helper; keep it even though current
// tests drive the loop manually to track metrics over time.
#[allow(dead_code)]
fn headless(ticks: u32) -> App {
    let mut app = make_app();
    for _ in 0..ticks {
        app.update();
    }
    app
}

fn elk_count(world: &mut World) -> usize {
    let mut q = world.query::<&Elk>();
    q.iter(world).count()
}

fn total_grass(world: &World) -> f32 {
    let grid = world.get_resource::<Grid>().unwrap();
    (0..grid.len()).map(|i| grid.grass(i)).sum()
}

fn max_col_reached(world: &mut World) -> usize {
    // Derive grid width without holding the &Grid borrow while the query borrows the world.
    let grid_width = world.get_resource::<Grid>().unwrap().width();
    let mut q = world.query::<&Elk>();
    q.iter(world).map(|elk| elk.cell % grid_width).max().unwrap_or(0)
}

// ── Invariant 1: population stays above zero and below an explosion ceiling ──

#[test]
fn population_does_not_collapse_or_explode() {
    const TICKS: u32 = 3000;

    let mut app = make_app();
    for tick in 0..TICKS {
        app.update();
        if tick % 500 == 499 {
            let count = elk_count(app.world_mut());
            println!("tick {}: elk_count = {count}", tick + 1);
        }
    }
    let count = elk_count(app.world_mut());
    println!("final (tick {TICKS}): elk_count = {count}");

    assert!(count > 0, "elk population collapsed to zero by tick {TICKS}");
    assert!(
        count < 8 * TARGET_POPULATION,
        "elk population exploded: {count} > {} at tick {TICKS}",
        8 * TARGET_POPULATION
    );
}

// ── Invariant 2: grass never gets fully eaten to bare dirt ──

#[test]
fn grass_never_fully_collapses() {
    const TICKS: u32 = 3000;
    // Skip the very first update: Bevy does not fire FixedUpdate on frame 0,
    // so grass legitimately starts at 0 before any growth tick has run.
    const WARMUP: u32 = 5;

    let mut app = make_app();
    let mut min_grass = f32::MAX;

    for tick in 0..TICKS {
        app.update();
        if tick >= WARMUP {
            let g = total_grass(app.world());
            if g < min_grass {
                min_grass = g;
            }
            if tick % 500 == 499 {
                println!("tick {}: total_grass = {g:.1}", tick + 1);
            }
        }
    }
    println!("min total_grass over ticks {WARMUP}..{TICKS} = {min_grass:.1}");

    // Observed baseline: min ~325 over 3000 ticks. Floor of 50 is a generous
    // collapse guard — six times below observed, tight enough to catch a near-wipeout.
    assert!(
        min_grass > 50.0,
        "grass nearly collapsed: min total_grass = {min_grass:.1} (floor = 50.0)"
    );
}

// ── Invariant 3: herds actually cross the map ──

#[test]
fn herds_reach_the_far_edge() {
    const TICKS: u32 = 3000;

    let mut app = make_app();
    let mut max_col_seen: usize = 0;

    for tick in 0..TICKS {
        app.update();
        let col = max_col_reached(app.world_mut());
        if col > max_col_seen {
            max_col_seen = col;
        }
        if tick % 500 == 499 {
            println!("tick {}: max_col_reached = {col}", tick + 1);
        }
    }
    println!("max_col_seen over {TICKS} ticks = {max_col_seen} (EDGE_COL = {EDGE_COL})");

    // Observed baseline: max_col_seen = 255 (elk pass EDGE_COL). Asserting >= 244
    // (EDGE_COL − 10) allows a 10-column margin while still detecting a failure
    // where migration pressure never builds.
    assert!(
        max_col_seen >= EDGE_COL - 10,
        "herds never reached the far edge: max_col_seen = {max_col_seen}, need >= {}",
        EDGE_COL - 10
    );
}
