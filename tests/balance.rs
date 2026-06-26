use mesopotamia::elk::ElkParams;
use mesopotamia::sim_harness::{journey_natural, run_metrics};

const TICKS: u32 = 300;

// doc02.02
const S_FLOOR: f32 = 0.1;
const EDGE_BAND: usize = 10;

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
