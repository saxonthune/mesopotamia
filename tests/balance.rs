use mesopotamia::elk::ElkParams;
use mesopotamia::sim_harness::{journey_natural, run_metrics};

/// Short horizon: fast enough for CI while still seeing survival and migration share settle.
const TICKS: u32 = 300;

// doc02.02 — thresholds start loose; tighten only when a real regression motivates it.
// At 300 ticks with migration=0 elk reach ~col 20+ on natural drives alone.
const S_FLOOR: f32 = 0.1;    // 10% minimum survival
const EDGE_BAND: usize = 10; // elk must move at least 10 cols on natural drives alone
const SIGMA_MAX: f32 = 0.99; // magic force must not be 100% of total pull

/// Assert the default `ElkParams` sit inside the loose balanced envelope from doc02.02.
#[test]
fn default_params_inside_balanced_envelope() {
    let params = ElkParams::default();
    let metrics = run_metrics(params.clone(), TICKS);

    println!(
        "survival={:.3}  max_col={}  mean_migration_share={:.3}",
        metrics.survival, metrics.max_col, metrics.mean_migration_share
    );

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

    assert!(
        metrics.mean_migration_share <= SIGMA_MAX,
        "mean_migration_share {:.3} exceeds sigma_max {} — migration dominates the drive",
        metrics.mean_migration_share,
        SIGMA_MAX
    );
}
