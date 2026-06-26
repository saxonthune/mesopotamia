//! Behaviour confirmation: each test runs a named demo `Preset` (the single source
//! the UI buttons also apply) over a purpose-built map, reduces the run to a metric
//! kernel from `mesopotamia::behaviors`, and asserts the success threshold. One
//! behaviour, one map, one metric — so a regression names which behaviour broke.

use mesopotamia::behaviors::{eastward_drift, is_circling, path_length, straightness};
use mesopotamia::elk::presets::{Preset, PRESETS};
use mesopotamia::sim_harness::{open_plain, river_plain, run_behavior, BehaviorTrace};

fn preset(name: &str) -> &'static Preset {
    PRESETS.iter().find(|p| p.name == name).expect("preset exists")
}

/// A square block of elk, all one cohort (slot 0), anchored with its west edge at
/// `col0` and centred vertically — the compact herd every behaviour starts from.
fn herd_block(width: usize, height: usize, col0: usize, side: usize) -> Vec<(usize, u8)> {
    let row0 = height / 2 - side / 2;
    let mut starts = Vec::new();
    for dr in 0..side {
        for dc in 0..side {
            starts.push(((row0 + dr) * width + (col0 + dc), 0u8));
        }
    }
    starts
}

fn report(name: &str, t: &BehaviorTrace) {
    let drift = eastward_drift(&t.centroid_col);
    let path = path_length(&t.centroid_col, &t.centroid_row);
    let straight = straightness(&t.centroid_col, &t.centroid_row);
    println!(
        "[{name}] drift={drift:.2} path={path:.2} straight={straight:.2} \
         final_energy={:.2} start_col={:.2} end_col={:.2}",
        t.final_energy(),
        t.centroid_col.first().copied().unwrap_or(0.0),
        t.centroid_col.last().copied().unwrap_or(0.0),
    );
}

/// Move in a direction: the persistent directional cause is the travelling green-up
/// wave (a static forage ramp is erased by regrowth within ~30 ticks). Under the
/// green wave the herd should follow the front east by a clear margin.
#[test]
fn herd_moves_in_a_direction() {
    let (w, h) = (40usize, 12usize);
    let grid = open_plain(w, h, 0.6);
    let starts = herd_block(w, h, 3, 4);
    let t = run_behavior(preset("Can cross"), grid, &starts, 0.6, 1500);
    report("direction", &t);
    assert!(
        eastward_drift(&t.centroid_col) > 4.0,
        "herd should follow the green wave east, drift was {:.2}",
        eastward_drift(&t.centroid_col)
    );
}

/// Cross a river: with the green-up wave on (Can cross preset) a herd on the near
/// bank should ford and end up east of the water.
#[test]
fn herd_crosses_a_river() {
    let (w, h) = (40usize, 12usize);
    let river_col = 18usize;
    let grid = river_plain(w, h, 0.7, river_col, h / 2);
    let starts = herd_block(w, h, 8, 4);
    let t = run_behavior(preset("Can cross"), grid, &starts, 0.6, 2000);
    report("crossing", &t);
    assert!(
        t.crossed_past(river_col) > 0.3,
        "at least a third of the herd should ford east of the river, was {:.2}",
        t.crossed_past(river_col)
    );
}

/// Don't go in circles: on a uniform, abundant plain with no directional cause the
/// herd has no reason to travel — the chew pin should hold it, so its centroid track
/// is short and is not a milling loop.
#[test]
fn idle_herd_does_not_circle() {
    let (w, h) = (32usize, 16usize);
    let grid = open_plain(w, h, 0.8);
    let starts = herd_block(w, h, 14, 4);
    let t = run_behavior(preset("Default"), grid, &starts, 0.6, 1200);
    report("idle", &t);
    assert!(
        !is_circling(&t.centroid_col, &t.centroid_row, 6.0, 0.4),
        "an idle herd milled in circles: path={:.2} straight={:.2}",
        path_length(&t.centroid_col, &t.centroid_row),
        straightness(&t.centroid_col, &t.centroid_row),
    );
}

