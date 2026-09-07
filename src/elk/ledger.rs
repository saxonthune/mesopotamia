use bevy::prelude::*;

use super::components::Herds;

/// Per-tick energy flows. Written by graze, metabolize, herd_step, spawn_waves, cull.
/// Reset before each tick by whichever consumer needs per-tick accounting.
#[derive(Resource, Default, Clone, Copy)]
pub struct EnergyFlows {
    pub intake: f32,
    pub drain: f32,
    pub swim: f32,
    pub births: f32,
    pub deaths_energy: f32,   // ≤ 0; below-zero overshoot at starvation
    pub departures_energy: f32, // ≥ 0; energy carried off the map
}

/// `alive + deaths + departures == spawned` for every cohort (after `tally_herds`).
#[allow(dead_code)]
pub fn population_balances(herds: &Herds) -> bool {
    herds.cohorts.values().all(|c| c.alive + c.deaths + c.departures == c.spawned)
}

/// Expected Δenergy this tick; conserved when `after - before == energy_expected_delta(flows)`.
/// Subtracting `deaths_energy` (≤ 0) closes the below-zero residual that vanishes on despawn.
#[allow(dead_code)]
pub fn energy_expected_delta(flows: &EnergyFlows) -> f32 {
    flows.intake - flows.drain - flows.swim + flows.births
        - flows.deaths_energy
        - flows.departures_energy
}

#[allow(dead_code)]
pub fn energy_ledger_closes(before: f32, after: f32, flows: &EnergyFlows, tol: f32) -> bool {
    (after - before - energy_expected_delta(flows)).abs() <= tol
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::elk::components::Cohort;

    fn herds_from(cohorts: Vec<(u32, u32, u32, u32)>) -> Herds {
        // (alive, deaths, departures, spawned)
        let mut h = Herds::default();
        for (i, (alive, deaths, departures, spawned)) in cohorts.into_iter().enumerate() {
            h.cohorts.insert(
                i as u32,
                Cohort { alive, deaths, departures, spawned, ..Default::default() },
            );
        }
        h
    }

    #[test]
    fn balanced_herds_pass() {
        let h = herds_from(vec![
            (10, 3, 2, 15), // 10+3+2 = 15 ✓
            (0, 5, 5, 10),  // 0+5+5  = 10 ✓
        ]);
        assert!(population_balances(&h));
    }

    #[test]
    fn unbalanced_cohort_fails() {
        let h = herds_from(vec![
            (10, 3, 2, 16), // 10+3+2 = 15 ≠ 16
        ]);
        assert!(!population_balances(&h));
    }

    #[test]
    fn empty_herds_balance() {
        assert!(population_balances(&Herds::default()));
    }

    #[test]
    fn balanced_tick_closes() {
        let flows = EnergyFlows {
            intake: 0.05,
            drain: 0.004,
            swim: 0.002,
            births: 1.0,
            deaths_energy: 0.0,
            departures_energy: 0.0,
        };
        let before = 5.0_f32;
        let after = before + energy_expected_delta(&flows);
        assert!(energy_ledger_closes(before, after, &flows, 1e-5));
    }

    #[test]
    fn unbalanced_tick_fails() {
        let flows = EnergyFlows {
            intake: 0.05,
            drain: 0.004,
            swim: 0.0,
            births: 0.0,
            deaths_energy: 0.0,
            departures_energy: 0.0,
        };
        let before = 5.0_f32;
        // after is wrong — drain is not subtracted
        let after = before + flows.intake;
        assert!(!energy_ledger_closes(before, after, &flows, 1e-4));
    }

    #[test]
    fn deaths_energy_accounts_for_below_zero_residual() {
        // Elk has energy 0.003, drain = 0.004:
        //   after drain: 0.003 - 0.004 = -0.001 → elk dies, deaths_energy = -0.001
        //   total_energy: before = 0.003, after = 0 (elk gone)
        //   actual_delta = -0.003
        //   expected:  intake(0) - drain(0.004) + births(0) - deaths_energy(-0.001) - departures(0)
        //            = -0.004 + 0.001 = -0.003 ✓
        let flows = EnergyFlows { drain: 0.004, deaths_energy: -0.001, ..Default::default() };
        assert!(energy_ledger_closes(0.003, 0.0, &flows, 1e-5));
    }

    #[test]
    fn departures_energy_accounts_for_exiting_elk() {
        // Elk departs with energy 0.7: energy leaves the ledger.
        //   total_energy: before = 0.7, after = 0 (elk gone)
        //   actual_delta = -0.7
        //   expected: -departures_energy(0.7) = -0.7 ✓
        let flows = EnergyFlows { departures_energy: 0.7, ..Default::default() };
        assert!(energy_ledger_closes(0.7, 0.0, &flows, 1e-5));
    }
}
