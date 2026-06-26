use mesopotamia::elk::ElkParams;
use mesopotamia::sim_harness::{
    make_probe_app, max_col_reached, probe_ablation, probe_grid, PROBE_FAR_BANK_COL,
    PROBE_FORD_ROW, PROBE_START_COL, PROBE_W,
};

fn probe_start_cell() -> usize {
    PROBE_FORD_ROW * PROBE_W + PROBE_START_COL
}

// RNG is seeded (ProbeSeed) so this is deterministic
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

// perception radius (not drive weight) gates crossing: blinding it delays/prevents the ford
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
