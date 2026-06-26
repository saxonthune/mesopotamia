use bevy::prelude::*;
use mesopotamia::elk::TARGET_POPULATION;
use mesopotamia::grid::Grid;
use mesopotamia::sim_harness::{elk_count, make_app};

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

// sum of per-cell carrying capacity — thresholds derived from it stay fair as grid/river layout varies
fn total_capacity(world: &World) -> f32 {
    let grid = world.get_resource::<Grid>().unwrap();
    (0..grid.len()).map(|i| grid.capacity(i)).sum()
}


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

#[test]
fn grass_never_fully_collapses() {
    const TICKS: u32 = 3000;
    // Bevy skips FixedUpdate on frame 0, so grass is 0 until after the first growth tick
    const WARMUP: u32 = 5;

    let mut app = make_app();
    let mut min_grass = f32::MAX;
    // read capacity after Startup runs; before first update it shows all-1.0 defaults, not the real layout
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
    // <2% of capacity is a near-wipeout; healthy runs sit ~5× above (observed min ~0.1 of ceiling)
    let floor = capacity * 0.02;
    println!(
        "min total_grass over ticks {WARMUP}..{TICKS} = {min_grass:.1} (capacity = {capacity:.1}, floor = {floor:.1})"
    );
    assert!(
        min_grass > floor,
        "grass nearly collapsed: min total_grass = {min_grass:.1} (floor = {floor:.1}, 5% of capacity {capacity:.1})"
    );
}

