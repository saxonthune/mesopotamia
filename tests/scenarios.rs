//! Headless scenario harness: play a controlled map through the real plugins, sample the
//! `WORLD_METRICS` into a `MetricLog`, then read it three ways — `summary()`/`to_csv()` for an
//! agent or console, and `series::*` for assertions. The one raw source, projected per concern.

use mesopotamia::behaviors::series;
use mesopotamia::elk::{ElkParams, RatioControls};
use mesopotamia::metrics::WORLD_METRICS;
use mesopotamia::sim_harness::{run, Scenario};
use mesopotamia::worldgen::testmap::{find, WorldSource};

fn oval_gaps_scenario() -> Scenario {
    Scenario {
        world: WorldSource::TestMap(find("oval-gaps").expect("oval-gaps map is registered")),
        params: ElkParams::default(),
        ratios: RatioControls::default(),
        spawns: vec![(0, 50)],
        ticks: 600,
    }
}

#[test]
fn oval_gaps_records_a_spawned_wave() {
    let log = run(&oval_gaps_scenario(), WORLD_METRICS);

    // Console/agent projections of the same raw log.
    println!("{}", log.summary());

    assert_eq!(log.rows.len(), 600, "one sampled row per tick");
    let pop = log.column("population");
    assert!(
        series::max(&pop) >= 45.0,
        "the scheduled 50-elk wave should appear in the population series (max was {})",
        series::max(&pop)
    );
    assert!(!log.column("centroid_col").is_empty(), "centroid is tracked");
}

/// Diagnostic (no tight gate yet — the movement mechanic is known-rough): prints the grazing
/// fingerprint an agent reads to spot "won't stop to eat / churns / settles all-at-once". The
/// numbers — low mean frac_grazing, high std + mean_crossings, flat intake — are the signature.
#[test]
fn oval_gaps_grazing_fingerprint() {
    let mut sc = oval_gaps_scenario();
    sc.ticks = 1200;
    let log = run(&sc, WORLD_METRICS);

    let graze = log.column("frac_grazing");
    let intake = log.column("intake_total");
    let gyr = log.column("gyration");
    println!(
        "frac_grazing: mean={:.3} std={:.3} min={:.3} max={:.3} mean_crossings={}",
        series::mean(&graze),
        series::std(&graze),
        series::min(&graze),
        series::max(&graze),
        series::mean_crossings(&graze),
    );
    println!(
        "intake_total: final={:.3} slope/tick={:.5}  |  gyration: mean={:.2} std={:.2}",
        series::final_value(&intake),
        series::slope(&intake),
        series::mean(&gyr),
        series::std(&gyr),
    );

    assert_eq!(log.rows.len(), 1200, "one row per tick");
    assert!(!graze.is_empty(), "frac_grazing is collected");
}

/// Not an assertion — a dumping ground so `just probe-scenario` emits the full CSV for offline
/// plotting. Ignored by default so the gate stays about behaviour, not output.
#[test]
#[ignore = "csv dump — run via `just probe-scenario`"]
fn oval_gaps_dump_csv() {
    let log = run(&oval_gaps_scenario(), WORLD_METRICS);
    print!("{}", log.to_csv());
}
