use mesopotamia::elk::ElkParams;
use mesopotamia::sim_harness::{
    make_probe_app, max_col_reached, probe_ablation, probe_grid, PROBE_FAR_BANK_COL,
    PROBE_FORD_ROW, PROBE_START_COL, PROBE_W,
};

fn probe_start_cell() -> usize {
    PROBE_FORD_ROW * PROBE_W + PROBE_START_COL
}

// ── Crossing probe ─────────────────────────────────────────────────────────────

/// A hungry elk on the near bank must cross the ford and reach the far-bank
/// forage within 200 ticks. The RNG is seeded (ProbeSeed) so this is deterministic.
#[test]
fn crossing_probe_elk_reaches_far_bank() {
    let mut app = make_probe_app(probe_grid(), &[(probe_start_cell(), 0)]);
    for _ in 0..200 {
        app.update();
    }
    let col = max_col_reached(app.world_mut());
    assert!(
        col >= PROBE_FAR_BANK_COL,
        "crossing probe: elk did not reach far bank after 200 ticks (max_col={col})"
    );
}

// ── Ablation probes ───────────────────────────────────────────────────────────

/// Cohesion is an inter-elk drive; a solo elk is unaffected. Zeroing it must not
/// prevent crossing — this tests the ablation infrastructure and confirms cohesion
/// does not gate the crossing.
#[test]
fn crossing_probe_ablation_cohesion_does_not_gate() {
    let ticks = probe_ablation(ElkParams::default(), |p| p.cohesion = 0.0, 200);
    assert!(
        ticks < 200,
        "ablation (cohesion=0): elk did not cross in 200 ticks (first crossed at tick={ticks})"
    );
}

/// Forage *perception* is what carries the elk across: the grass-gradient signal
/// reaches over `grass_radius` cells to sense the far-bank forage, pulling the elk
/// into Travel toward the ford. Collapsing that radius to a single cell blinds the
/// elk to the far bank, so it never commits to the crossing — pinning perception,
/// not any drive weight, as the gate under the herding model.
#[test]
fn crossing_probe_forage_perception_gates_crossing() {
    let ticks_full = probe_ablation(ElkParams::default(), |_| {}, 500);
    let ticks_blind = probe_ablation(ElkParams::default(), |p| p.grass_radius = 1.0, 500);

    assert!(
        ticks_full < 500,
        "baseline crossing probe failed: elk did not cross in 500 ticks"
    );
    assert!(
        ticks_full < ticks_blind,
        "blinding forage perception must delay crossing: baseline={ticks_full}, blind={ticks_blind}"
    );
}
