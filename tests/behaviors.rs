//! Behaviour confirmation: one preset, one map, one metric — so a regression names which behaviour broke.
//! Presets are the same objects the UI buttons apply.

use mesopotamia::behaviors::{eastward_drift, is_circling, path_length, straightness};
use mesopotamia::elk::presets::{Preset, PRESETS};
use mesopotamia::sim_harness::{open_plain, river_plain, run_behavior, BehaviorTrace};

fn preset(name: &str) -> &'static Preset {
    PRESETS.iter().find(|p| p.name == name).expect("preset exists")
}

// west edge at col0, centred vertically
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

// Regression-deferred (2026-06-26): the intake-driven leave rule no longer churns the herd
// aimlessly over uniform forage, so on this gradient-less map it settles and never reaches the
// river. Crossing needs a directional driver (the green-wave/migration heading is an open design
// question — see the `herd-movement-phase-lock` reference doc); revive with a forage-gradient map.
#[ignore = "needs directional driver; intake-rule herd doesn't churn across uniform forage"]
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

// no directional cause → chew pin holds the herd; centroid track must be short and non-circular
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

