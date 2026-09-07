//! Movement lab — prints the lattice tick-by-tick via `ascii_frame` so spatial bugs are visible directly.
//! All tests are `#[ignore]`'d (human/agent debug surface, not gates); herding is deterministic so every run replays identically.

use bevy::prelude::*;
use mesopotamia::elk::{ElkParams, HerdParams};
use mesopotamia::sim_harness::{
    ascii_frame, make_probe_app, open_plain, river_plain, run_migration,
};

fn watch(name: &str, app: &mut App, ticks: u32, every: u32) {
    println!("\n=== {name}: tick 0 (initial) ===\n{}", ascii_frame(app.world_mut()));
    for t in 0..ticks {
        app.update();
        let n = t + 1;
        if n % every == 0 || n == ticks {
            println!("=== {name}: tick {n} ===\n{}", ascii_frame(app.world_mut()));
        }
    }
}

fn feed_all(app: &mut App, energy: f32) {
    let world = app.world_mut();
    let mut q = world.query::<&mut mesopotamia::elk::Elk>();
    for mut elk in q.iter_mut(world) {
        elk.energy = energy;
    }
}

const W: usize = 24;
const H: usize = 14;

// reported bug: kitty-corner pair "try to pass through each other but get stuck"
#[test]
#[ignore = "movement lab — run manually with --ignored --nocapture"]
fn lab_kitty_corner_pair() {
    let a = 6 * W + 6;
    let b = 7 * W + 7;
    let grid = open_plain(W, H, 1.0);
    let mut app = make_probe_app(grid, &[(a, 0), (b, 0)]);
    app.insert_resource(ElkParams::default());
    app.insert_resource(HerdParams::default());
    feed_all(&mut app, 0.8);
    watch("kitty", &mut app, 20, 1);
}

// no forage gradient — isolates separation+cohesion; herd should hold a loose clump, not diffuse
#[test]
#[ignore = "movement lab — run manually with --ignored --nocapture"]
fn lab_herd_on_plain() {
    let mut starts = Vec::new();
    for row in 5..9 {
        for col in 5..9 {
            starts.push((row * W + col, 0u8));
        }
    }
    let grid = open_plain(W, H, 1.0);
    let mut app = make_probe_app(grid, &starts);
    app.insert_resource(ElkParams::default());
    app.insert_resource(HerdParams::default());
    feed_all(&mut app, 0.8);
    watch("plain", &mut app, 120, 20);
}

const HW: usize = 32;
const HH: usize = 12;

fn happy_map() -> (mesopotamia::grid::Grid, Vec<(usize, u8)>) {
    let river_col = HW / 2;
    let ford_row = HH / 2;
    let mut starts = Vec::new();
    for row in 3..9 {
        for col in 1..4 {
            starts.push((row * HW + col, 0u8));
        }
    }
    let mut grid = river_plain(HW, HH, 0.3, river_col, ford_row);
    for row in 0..HH {
        for col in 0..HW {
            if col == river_col {
                continue;
            }
            let cell = row * HW + col;
            let frac = 0.25 + 0.65 * (col as f32 / (HW - 1) as f32);
            grid.set_grass(cell, frac * grid.capacity(cell));
        }
    }
    (grid, starts)
}

// west→east gradient + river ford + east-edge despawn; promote to a gate once reliably passing
#[test]
#[ignore = "movement lab — run manually with --ignored --nocapture"]
fn lab_happy_path() {
    const TICKS: u32 = 2000;
    let (grid, starts) = happy_map();
    let report = run_migration(grid, &starts, ElkParams::default(), HerdParams::default(), 1.0, TICKS);

    println!("\n=== happy-path report ({TICKS} ticks) ===");
    println!("start_pop   : {}", report.start_pop);
    println!("start_col   : {:.1}", report.start_col);
    println!("end_col     : {:.1}", report.end_col);
    println!("max_col     : {}  (edge band starts at col {})", report.max_col, report.edge_col);
    println!("reached_edge: {}", report.reached_edge());
    println!("departures  : {}", report.departures);
    println!("deaths      : {}", report.deaths);
    println!("alive (stuck): {}", report.alive);
    println!("all_departed: {}", report.all_departed());
}

// distinguishes tuning failure from structural stall: if every cell stalls across the sweep, the mechanism is broken
#[test]
#[ignore = "movement lab — run manually with --ignored --nocapture"]
fn lab_happy_sweep() {
    const TICKS: u32 = 2000;
    println!("\nconf_ref  cohesion  end_col  max_col  depart  deaths  alive");
    for &conf_ref in &[0.5_f32, 0.1, 0.03, 0.01] {
        for &cohesion in &[0.5_f32, 0.2, 0.05] {
            let (grid, starts) = happy_map();
            let herd = HerdParams { confidence_ref: conf_ref, cohesion, ..Default::default() };
            let r = run_migration(grid, &starts, ElkParams::default(), herd, 1.0, TICKS);
            println!(
                "{conf_ref:>7.2}  {cohesion:>7.2}  {:>6.1}  {:>6}  {:>5}  {:>5}  {:>5}",
                r.end_col, r.max_col, r.departures, r.deaths, r.alive
            );
        }
    }
}

// watch Travel→Cross handoff: does the herd funnel to the ford or pile up against the water?
#[test]
#[ignore = "movement lab — run manually with --ignored --nocapture"]
fn lab_herd_crosses_river() {
    let river_col = W / 2;
    let ford_row = H / 2;
    let mut starts = Vec::new();
    for row in 5..9 {
        for col in 2..5 {
            starts.push((row * W + col, 0u8));
        }
    }
    // Far bank lush, near bank thin — a forage reason to cross.
    let mut grid = river_plain(W, H, 0.3, river_col, ford_row);
    for row in 0..H {
        for col in (river_col + 1)..W {
            grid.set_grass(row * W + col, grid.capacity(row * W + col));
        }
    }
    let mut app = make_probe_app(grid, &starts);
    app.insert_resource(ElkParams::default());
    app.insert_resource(HerdParams::default());
    feed_all(&mut app, 0.8);
    watch("river", &mut app, 200, 25);
}
