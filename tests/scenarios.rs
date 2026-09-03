//! Headless scenario harness: play a controlled map through the real plugins, sample the
//! `WORLD_METRICS` into a `MetricLog`, then read it three ways — `summary()`/`to_csv()` for an
//! agent or console, and `series::*` for assertions. The one raw source, projected per concern.

use mesopotamia::behaviors::series;
use mesopotamia::elk::{ElkParams, HerdParams, RatioControls};
use mesopotamia::metrics::WORLD_METRICS;
use mesopotamia::sim_harness::{run, Scenario};
use mesopotamia::worldgen::testmap::{find, WorldSource};

fn oval_gaps_scenario() -> Scenario {
    Scenario {
        world: WorldSource::TestMap(find("oval-gaps").expect("oval-gaps map is registered")),
        params: ElkParams::default(),
        ratios: RatioControls::default(),
        herd: HerdParams::default(),
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
    let ccol = log.column("centroid_col");
    let crow = log.column("centroid_row");
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
    // Spatial phase-lock signal: a directed herd trends, a sloshing herd reverses. eps≈0.05 cell
    // ignores sub-cell jitter. Net col travel vs path of reversals distinguishes the two.
    println!(
        "centroid: col {:.1}→{:.1} (net {:+.1})  reversals col={} row={}",
        series::final_value(&ccol[..1].to_vec()),
        series::final_value(&ccol),
        series::final_value(&ccol) - ccol.first().copied().unwrap_or(0.0),
        series::reversals(&ccol, 0.05),
        series::reversals(&crow, 0.05),
    );

    assert_eq!(log.rows.len(), 1200, "one row per tick");
    assert!(!graze.is_empty(), "frac_grazing is collected");
}

/// Diagnostic sweep: how onward migration vs. in-place sloshing vs. starvation trade off against
/// shrub regrowth. net_col ≈ how many of the 5 ovals (centres ~44 cells apart) the herd crossed;
/// reversals = sloshing; final_pop = did they starve. The sweet spot is high net_col, low
/// reversals, surviving pop. Ignored by default — run explicitly to retune.
#[test]
#[ignore = "diagnostic sweep — run explicitly"]
fn oval_gaps_regrow_sweep() {
    for shrub_regrow in [0.0f32, 0.0005, 0.001, 0.0015, 0.002, 0.0025] {
        let mut sc = oval_gaps_scenario();
        sc.ticks = 1200;
        sc.ratios.shrub_regrow = shrub_regrow;
        let log = run(&sc, WORLD_METRICS);
        let ccol = log.column("centroid_col");
        let pop = log.column("population");
        println!(
            "shrub_regrow={:.4}: net_col={:+6.1}  reversals_col={:3}  final_pop={:3.0}  intake_slope={:.5}",
            shrub_regrow,
            series::final_value(&ccol) - ccol.first().copied().unwrap_or(0.0),
            series::reversals(&ccol, 0.05),
            series::final_value(&pop),
            series::slope(&log.column("intake_total")),
        );
    }
}

/// Diagnostic: is onward migration merely slow (patch too rich for the run) or absent? Run long
/// enough to strip oval 1, sampling the centroid column every 800 ticks. If net_col climbs past
/// ~70 (oval 2 centre) the depletion→migrate loop works and the fix is sizing, not mechanism.
/// Validation for the extended (omnidirectional, distance-discounted, gated) perception: with the
/// right-sized ovals (~25 cells apart) and depleting regrowth, a herd should *chain* patch-to-patch
/// — the centroid climbing through ~26, ~51, ~76, … as each patch strips and the next is perceived.
/// Flat col ≈ stuck at oval 1; a steady climb with low reversals ≈ the jump mechanism works.
#[test]
#[ignore = "diagnostic long run — run explicitly"]
fn oval_gaps_long_migration() {
    for shrub_regrow in [0.001f32, 0.0015, 0.002, 0.0025] {
        let mut sc = oval_gaps_scenario();
        sc.ticks = 5000;
        sc.ratios.shrub_regrow = shrub_regrow;
        sc.spawns = vec![(0, 100)];
        let log = run(&sc, WORLD_METRICS);
        let ccol = log.column("centroid_col");
        let n = ccol.len();
        let at = |t: usize| ccol.get(t.min(n.saturating_sub(1))).copied().unwrap_or(0.0);
        println!(
            "regrow={shrub_regrow:.4}: col@1000={:.0} @2000={:.0} @3000={:.0} @4000={:.0} @end={:.0}  reversals={} final_pop={:.0}",
            at(1000), at(2000), at(3000), at(4000), series::final_value(&ccol),
            series::reversals(&ccol, 0.05),
            series::final_value(&log.column("population")),
        );
    }
}

/// Not an assertion — a dumping ground so `just probe-scenario` emits the full CSV for offline
/// plotting. Ignored by default so the gate stays about behaviour, not output.
#[test]
#[ignore = "csv dump — run via `just probe-scenario`"]
fn oval_gaps_dump_csv() {
    let log = run(&oval_gaps_scenario(), WORLD_METRICS);
    print!("{}", log.to_csv());
}
