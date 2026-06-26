use mesopotamia::elk::{energy_expected_delta, energy_ledger_closes, population_balances, EnergyFlows, Herds};
use mesopotamia::sim_harness::{elk_flows, make_app, total_elk_energy};

// checked after the full run so tally_herds has had time to recount every cohort
#[test]
fn population_identity_holds_after_run() {
    const TICKS: u32 = 300;

    let mut app = make_app();
    for _ in 0..TICKS {
        app.update();
    }

    let world = app.world();
    let herds = world.get_resource::<Herds>().unwrap();

    for (code, c) in &herds.cohorts {
        let sum = c.alive + c.deaths + c.departures;
        if sum != c.spawned {
            println!(
                "IMBALANCE cohort {:06x}: alive={} deaths={} departures={} sum={} spawned={}",
                code, c.alive, c.deaths, c.departures, sum, c.spawned
            );
        }
    }
    assert!(population_balances(herds), "population identity violated — see cohort breakdown above");
}

#[test]
fn energy_ledger_closes_each_tick() {
    const TICKS: u32 = 300;
    // ~1e-7 relative error × ~200 elk × 300 ticks accumulates well under 0.01; tight enough to catch a real leak
    const TOL_PER_TICK: f32 = 0.01;

    let mut app = make_app();

    let mut max_error: f32 = 0.0;
    let mut worst_tick: u32 = 0;
    let mut worst_flows = EnergyFlows::default();
    let mut worst_before = 0.0_f32;
    let mut worst_after = 0.0_f32;

    for tick in 0..TICKS {
        // Reset flows before this tick so we get only this tick's accumulation.
        *app.world_mut().resource_mut::<EnergyFlows>() = EnergyFlows::default();
        let before = total_elk_energy(app.world());

        app.update();

        let after = total_elk_energy(app.world());
        let flows = elk_flows(app.world());

        if before > 0.0 || flows.births > 0.0 {
            let expected = energy_expected_delta(&flows);
            let error = (after - before - expected).abs();

            if error > max_error {
                max_error = error;
                worst_tick = tick + 1;
                worst_flows = flows;
                worst_before = before;
                worst_after = after;
            }
        }
    }

    println!(
        "energy ledger over {TICKS} ticks: max_error={max_error:.7} at tick {worst_tick} \
         (before={worst_before:.4} after={worst_after:.4} \
         intake={:.4} drain={:.4} swim={:.4} births={:.4} \
         deaths_energy={:.4} departures_energy={:.4})",
        worst_flows.intake,
        worst_flows.drain,
        worst_flows.swim,
        worst_flows.births,
        worst_flows.deaths_energy,
        worst_flows.departures_energy,
    );

    assert!(
        energy_ledger_closes(worst_before, worst_after, &worst_flows, TOL_PER_TICK),
        "energy ledger did not close: max per-tick error {max_error:.7} exceeds tolerance {TOL_PER_TICK}"
    );
}
