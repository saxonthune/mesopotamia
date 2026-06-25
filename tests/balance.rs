use mesopotamia::elk::ElkParams;
use mesopotamia::sim_harness::{journey_natural, run_metrics};

/// Short horizon: fast enough for CI while still seeing survival settle.
const TICKS: u32 = 300;

// doc02.02 — thresholds start loose; tighten only when a real regression motivates it.
// At 300 ticks elk reach ~col 20+ on the forage drives alone.
const S_FLOOR: f32 = 0.1;    // 10% minimum survival
const EDGE_BAND: usize = 10; // elk must move at least 10 cols on the forage drives alone

/// Assert the default `ElkParams` sit inside the loose balanced envelope from doc02.02.
#[test]
fn default_params_inside_balanced_envelope() {
    let params = ElkParams::default();
    let metrics = run_metrics(params.clone(), TICKS);

    println!("survival={:.3}  max_col={}", metrics.survival, metrics.max_col);

    assert!(
        metrics.survival >= S_FLOOR,
        "survival {:.3} is below the floor {} — too many elk starved",
        metrics.survival,
        S_FLOOR
    );

    let nat = journey_natural(params, TICKS);
    println!("journey_natural max_col={nat} (edge_band={EDGE_BAND})");
    assert!(
        nat >= EDGE_BAND,
        "journey_natural={nat} did not reach edge_band={EDGE_BAND} — naturals can't carry the herd"
    );
}
