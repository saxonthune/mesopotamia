use bevy::prelude::*;
use mesopotamia::elk::TARGET_POPULATION;
use mesopotamia::grid::Grid;
use mesopotamia::sim_harness::{elk_count, make_app};

/// Build a headless app, step it `ticks` times, and return the final state.
#[allow(dead_code)]
fn headless(ticks: u32) -> App {
    let mut app = make_app();
    for _ in 0..ticks {
        app.update();
    }
    app
}

fn total_grass(world: &World) -> f32 {
    let grid = world.get_resource::<Grid>().unwrap();
    (0..grid.len()).map(|i| grid.grass(i)).sum()
}

/// The world's total grass ceiling — sum of per-cell carrying capacity. Scales
/// with both grid size and the river/soil layout, so thresholds derived from it
/// stay fair when either changes.
fn total_capacity(world: &World) -> f32 {
    let grid = world.get_resource::<Grid>().unwrap();
    (0..grid.len()).map(|i| grid.capacity(i)).sum()
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
    // The grass ceiling depends only on the static water/soil fields, captured once
    // after Startup (the river/soil generators) has run. Reading it before the first
    // update would see the all-1.0 defaults, not the real layout.
    let mut capacity = 0.0_f32;

    for tick in 0..TICKS {
        app.update();
        if tick == WARMUP {
            capacity = total_capacity(app.world());
        }
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
    // Collapse guard as a fraction of the world's grass ceiling, so it stays a fair
    // test at any grid size or river layout. A standing crop below 2% of capacity is
    // a near-wipeout; healthy runs sit ~5× above it (observed min ~0.1 of ceiling).
    let floor = capacity * 0.02;
    println!(
        "min total_grass over ticks {WARMUP}..{TICKS} = {min_grass:.1} (capacity = {capacity:.1}, floor = {floor:.1})"
    );
    assert!(
        min_grass > floor,
        "grass nearly collapsed: min total_grass = {min_grass:.1} (floor = {floor:.1}, 5% of capacity {capacity:.1})"
    );
}

// Note: the former "herds reach the far edge" invariant was retired. The demo's
// default is now an intentionally milling herd that does not cross unaided; the
// live invariant — that good tuning carries the herd far across — lives in
// `tests/herd_shape.rs::tuned_weights_cross_much_further_than_stock`.
