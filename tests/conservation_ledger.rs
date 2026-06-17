use mesopotamia::elk::{energy_expected_delta, energy_ledger_closes, population_balances, EnergyFlows, Herds};
use mesopotamia::sim_harness::{elk_flows, make_app, total_elk_energy};

// ── Invariant 1: population conservation identity ──────────────────────────

/// For every cohort, alive + deaths + departures must equal the number of elk
/// originally spawned in that cohort's wave. Checked after the full run so that
/// tally_herds has had time to recount every cohort.
#[test]
fn population_identity_holds_after_run() {
    const TICKS: u32 = 300;

    let mut app = make_app();
    for _ in 0..TICKS {
        app.update();
    }

    let world = app.world();
    let herds = world.get_resource::<Herds>().unwrap();

    // Print cohort totals for inspection on failure.
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

// ── Invariant 2: energy ledger closes per tick ─────────────────────────────

/// Run a headless sim for N ticks and assert the energy ledger closes each
/// tick within a small floating-point tolerance. All source/sink flows are
/// tracked by the simulation systems (intake, drain, swim, births, starvation
/// residuals, departure energy), so the expected delta should match the
/// measured delta with only floating-point rounding error.
///
/// If a genuine leak is found the test will fail with the accumulated error,
/// pointing to the leaky flow.
#[test]
fn energy_ledger_closes_each_tick() {
    const TICKS: u32 = 300;
    // Floating-point rounding across many elk and operations accumulates error.
    // Each operation has ~1e-7 relative error; with ~200 elk over 300 ticks,
    // the expected accumulated drift is well under 0.01. A tolerance of 0.01
    // per tick is tight enough to catch a genuine leak.
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

        // Only check ticks where something is happening.
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
