//! Herd-shape soak tests on a controlled dummy map.
//!
//! On a flat, evenly-stocked open plain the terrain explains nothing — the only
//! thing shaping the herd is the decision model. So these assert the model's
//! intent directly: a herd dropped on abundant grass must *feed and survive*, not
//! clump to a point and starve. The per-tick `RunTrace` is printed so a failure
//! shows the shape of the collapse (gyration → 0, energy falling, everyone stuck
//! in travel mode), not just a red assertion.

use mesopotamia::elk::{ElkParams, RatioControls, Score};
use mesopotamia::events::{EventKind, EventLog};
use mesopotamia::grid::GreenWave;
use mesopotamia::sim_harness::{
    centroid_col, diagnose, diagnose_worldgen, elk_count, make_app, make_probe_app, max_col_reached,
    open_plain, spawner_elapsed, total_elk_energy,
};

// The demo's central invariant, pinned: tuning toward forage + committing the
// migration pull carries the herd dramatically further across the map than the
// stock "mill in place" weights. The 3.5× gap (≈160 vs ≈45 of 256) swamps the
// run-to-run noise of the unseeded worldgen, so a generous absolute margin holds.
// If this ever fails, the demo's core lever — that good tuning visibly wins — is
// broken, regardless of what any gauge says.
#[test]
fn tuned_weights_cross_much_further_than_stock() {
    const TICKS: u32 = 1000;
    let reach = |cross_ratio: f32, tweak: fn(&mut ElkParams)| -> usize {
        let mut app = make_app();
        app.insert_resource(RatioControls { cross_ratio, ..Default::default() });
        let mut p = app.world().resource::<ElkParams>().clone();
        tweak(&mut p);
        app.insert_resource(p);
        let mut max_col = 0usize;
        for _ in 0..TICKS {
            app.update();
            max_col = max_col.max(max_col_reached(app.world_mut()));
        }
        max_col
    };
    let stock = reach(0.10, |_p| {});
    let tuned = reach(1.00, |p| { p.grass = 2.5; p.social = 2.0; p.grass_radius = 10.0; p.temperature = 0.4; });
    println!("stock reach={stock}/256, tuned reach={tuned}/256");
    assert!(
        tuned > stock + 50,
        "tuned weights+pull should cross far further than stock: tuned={tuned}, stock={stock}"
    );
}

// Searches the score's two biggest levers — scarcity (difficulty) and the pull
// (forward progress) — with a strong foraging base, and reports the high score each
// reaches. The peak names the "optimal weights". Single unseeded runs, so read the
// region, not the exact cell.
#[test]
#[ignore = "score optimization sweep, run explicitly with --nocapture"]
fn optimal_weight_sweep() {
    const TICKS: u32 = 3000;
    let forager: fn(&mut ElkParams) = |p| {
        p.grass = 2.5;
        p.social = 2.0;
        p.grass_radius = 10.0;
        p.temperature = 0.4;
    };
    let mut rows: Vec<(f32, f32, f32, f32)> = Vec::new();
    for regrow_ratio in [0.04_f32, 0.07, 0.10, 0.14] {
        for cross_ratio in [0.5_f32, 1.0, 2.0] {
            let mut app = make_app();
            app.insert_resource(RatioControls { cross_ratio, regrow_ratio, ..Default::default() });
            let mut p = app.world().resource::<ElkParams>().clone();
            forager(&mut p);
            app.insert_resource(p);
            for _ in 0..TICKS { app.update(); }
            let s = app.world().resource::<Score>();
            rows.push((regrow_ratio, cross_ratio, s.difficulty, s.high));
        }
    }
    rows.sort_by(|a, b| b.3.partial_cmp(&a.3).unwrap());
    println!("--- high score, best first ---");
    for (regrow, cross, diff, high) in rows {
        println!("regrow={regrow:.2} cross={cross:.1} difficulty={diff:.2} high={high:.0}");
    }
}

// At the score optimum (max scarcity + max pull), which weight set scores best?
// Tells us whether foraging/dispersal tuning matters there or it's all the pull.
#[test]
#[ignore = "weight refinement at the optimum, run explicitly with --nocapture"]
fn optimal_weight_refine() {
    const TICKS: u32 = 3000;
    let variants: [(&str, fn(&mut ElkParams)); 6] = [
        ("forager", |p| { p.grass = 2.5; p.social = 2.0; p.grass_radius = 10.0; p.temperature = 0.4; }),
        ("grass-max", |p| { p.grass = 3.0; p.social = 2.0; p.grass_radius = 12.0; p.temperature = 0.4; }),
        ("cold", |p| { p.grass = 2.5; p.social = 2.0; p.grass_radius = 10.0; p.temperature = 0.2; }),
        ("disperse", |p| { p.grass = 2.5; p.social = 2.0; p.grass_radius = 10.0; p.temperature = 0.4; p.separation = 2.5; p.cohesion = 0.3; }),
        ("social-max", |p| { p.grass = 2.0; p.social = 3.0; p.grass_radius = 10.0; p.temperature = 0.4; }),
        ("lean", |p| { p.grass = 1.0; p.social = 0.5; p.temperature = 0.4; }),
    ];
    let mut rows: Vec<(&str, f32)> = Vec::new();
    for (name, tweak) in variants {
        let mut app = make_app();
        app.insert_resource(RatioControls { cross_ratio: 2.0, regrow_ratio: 0.04, ..Default::default() });
        let mut p = app.world().resource::<ElkParams>().clone();
        tweak(&mut p);
        app.insert_resource(p);
        for _ in 0..TICKS { app.update(); }
        rows.push((name, app.world().resource::<Score>().high));
    }
    rows.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    println!("--- high score at regrow=0.04 cross=2.0, best first ---");
    for (name, high) in rows { println!("{name:>10}: high={high:.0}"); }
}

// Long-run check that the +1 departure outcome actually fires and the rating
// discriminates: over a realistic horizon, a tuned herd with a real pull should
// cross the far edge (departures > 0) and rate above a milling default.
#[test]
#[ignore = "long-run score probe, run explicitly with --nocapture"]
fn score_longrun_probe() {
    const TICKS: u32 = 4000;
    let run = |name: &str, cross_ratio: f32, regrow_ratio: f32, tweak: fn(&mut ElkParams)| {
        let mut app = make_app();
        app.insert_resource(RatioControls { cross_ratio, regrow_ratio, ..Default::default() });
        let mut p = app.world().resource::<ElkParams>().clone();
        tweak(&mut p);
        app.insert_resource(p);
        for _ in 0..TICKS { app.update(); }
        let log = app.world().resource::<EventLog>();
        let departed = log.recent.iter().filter(|e| matches!(e.kind, EventKind::Departed)).count();
        let starved = log.recent.iter().filter(|e| matches!(e.kind, EventKind::Starved)).count();
        let score = app.world().resource::<Score>();
        println!(
            "{name:>8}: departed={departed:>4} starved={starved:>4} difficulty={:.2} current={:.0} high={:.0}",
            score.difficulty, score.current, score.high
        );
    };
    run("default", 0.10, 0.175, |_p| {});
    run("happy", 1.00, 0.175, |p| { p.grass = 2.5; p.social = 2.0; p.grass_radius = 10.0; p.temperature = 0.4; });
    run("scarce", 0.80, 0.060, |p| { p.grass = 2.5; p.social = 2.0; p.grass_radius = 10.0; p.temperature = 0.4; });
}

// The three reference regimes the demo is built around, on the real map. Run with
// --nocapture to read them; they become assertion gates once the numbers settle.
//   default   — stock weights, weak pull, plenty of food: the herd mills and
//               overcrowds the first segment instead of crossing (the intended
//               initial experience).
//   happy     — tuned foraging + a real pull + plenty of food: the herd crosses
//               far with few deaths (the "it works" baseline).
//   optimized — tuned foraging + minimal pull under self-imposed scarcity: the
//               power-gamer target — survive the famine AND still push forward.
#[test]
#[ignore = "reference cases, run explicitly with --nocapture"]
fn three_reference_cases() {
    const TICKS: u32 = 1000;
    const WIN: u64 = 200;
    // (name, regrow_ratio, cross_ratio, weight tweak)
    let cases: [(&str, f32, f32, fn(&mut ElkParams)); 3] = [
        ("default", 0.175, 0.10, |_p| {}),
        ("happy", 0.175, 1.00, |p| { p.grass = 2.5; p.social = 2.0; p.grass_radius = 10.0; p.temperature = 0.4; }),
        ("optimized", 0.060, 0.50, |p| { p.grass = 2.5; p.social = 2.0; p.grass_radius = 10.0; p.temperature = 0.4; p.separation = 1.5; }),
    ];
    for (name, regrow_ratio, cross_ratio, tweak) in cases {
        let mut app = make_app();
        app.insert_resource(RatioControls { cross_ratio, regrow_ratio, ..Default::default() });
        {
            let mut p = app.world().resource::<ElkParams>().clone();
            tweak(&mut p);
            app.insert_resource(p);
        }
        let (mut max_col, mut e_sum, mut e_ticks) = (0usize, 0.0_f32, 0u32);
        for _ in 0..TICKS {
            app.update();
            max_col = max_col.max(max_col_reached(app.world_mut()));
            let n = elk_count(app.world_mut());
            if n > 0 { e_sum += total_elk_energy(app.world()) / n as f32; e_ticks += 1; }
        }
        let now = spawner_elapsed(app.world()) as u64;
        let deaths = app.world().resource::<EventLog>().recent.iter()
            .filter(|e| matches!(e.kind, EventKind::Starved) && e.tick + WIN > now).count();
        let score = app.world().resource::<Score>();
        let (current, high, difficulty) = (score.current, score.high, score.difficulty);
        let n = elk_count(app.world_mut());
        let mean_e = if e_ticks > 0 { e_sum / e_ticks as f32 } else { 0.0 };
        println!("{name:>9}: max_col={max_col:>3}/256 final_pop={n:>3} mean_energy={mean_e:.3} deaths/{WIN}t={deaths} difficulty={difficulty:.2} current={current:.0} high={high:.0}");
    }
}

// Does skillful tuning measurably beat poor tuning under self-imposed scarcity,
// with no magic pull? If good weights cut deaths and raise reach, the efficiency
// score has a real strategy space (we're set up). If every strategy collapses
// the same way, reach/spread isn't a responsive lever and needs a mechanic.
#[test]
#[ignore = "design probe, run explicitly with --nocapture"]
fn scarcity_strategy_grid() {
    const TICKS: u32 = 800;
    const WIN: u64 = 200;
    let strategies: [(&str, fn(&mut ElkParams)); 3] = [
        ("baseline", |_p| {}),
        ("forager", |p| { p.grass = 2.5; p.social = 2.0; p.dwell = 2.5; p.grass_radius = 10.0; }),
        ("nomad", |p| { p.separation = 2.5; p.cohesion = 0.2; p.grass = 2.0; p.leave_frac = 0.6; }),
    ];
    for regrow_ratio in [0.175_f32, 0.06] {
        for (name, tweak) in &strategies {
            let mut app = make_app();
            app.insert_resource(RatioControls { cross_ratio: 0.0, regrow_ratio, ..Default::default() });
            {
                let mut p = app.world().resource::<ElkParams>().clone();
                tweak(&mut p);
                app.insert_resource(p);
            }
            let mut max_col = 0usize;
            for _ in 0..TICKS {
                app.update();
                max_col = max_col.max(max_col_reached(app.world_mut()));
            }
            let now = spawner_elapsed(app.world()) as u64;
            let deaths = app
                .world()
                .resource::<EventLog>()
                .recent
                .iter()
                .filter(|e| matches!(e.kind, EventKind::Starved) && e.tick + WIN > now)
                .count();
            let n = elk_count(app.world_mut());
            println!("regrow={regrow_ratio:.3} {name:>9}: final_pop={n:>3} max_col={max_col:>3}/256 deaths/{WIN}t={deaths}");
        }
    }
}

// Calibration aid (run with --nocapture): how the two challenge meters read at
// default weights, with the magic pull on vs off. Confirms the pull gauge sits
// in the red at default (the herd leans on the crutch) and the starvation gauge
// spikes when the crutch is removed without good foraging weights.
#[test]
#[ignore = "calibration aid, run explicitly with --nocapture"]
fn default_meters_diagnostic() {
    const TICKS: u32 = 800;
    // Each attempt sets migration to zero (cross_ratio 0 — no magic pull) and tries
    // a different natural-weight strategy. Question: can ANY of them cross the map?
    let attempts: [(&str, fn(&mut ElkParams)); 5] = [
        ("defaults", |_p| {}),
        ("high-grass+hot", |p| { p.grass = 3.0; p.temperature = 1.5; }),
        ("disperse+grass", |p| { p.separation = 3.0; p.cohesion = 0.1; p.grass = 3.0; }),
        ("social+grass", |p| { p.social = 3.0; p.grass = 3.0; }),
        ("hot+farsight", |p| { p.temperature = 2.0; p.grass = 3.0; p.grass_radius = 12.0; }),
    ];
    for (name, tweak) in attempts {
        let mut app = make_app();
        app.insert_resource(RatioControls { cross_ratio: 0.0, ..Default::default() });
        {
            let mut p = app.world().resource::<ElkParams>().clone();
            tweak(&mut p);
            app.insert_resource(p);
        }
        let mut max_col = 0usize;
        for _ in 0..TICKS {
            app.update();
            max_col = max_col.max(max_col_reached(app.world_mut()));
        }
        let n = elk_count(app.world_mut());
        println!("[no-pull] {name:>16}: final_pop={n:>3} max_col={max_col}/256");
    }
}

// Tuning aid (run with --nocapture): sweep the food-economy ratio on the real
// map and report whether the herd stays mid-fed (foraging matters but it can
// sustain) versus overfed-and-idle or starving. Not a gate — a knob-finder.
#[test]
#[ignore = "tuning aid, run explicitly with --nocapture"]
fn bite_ratio_sweep_diagnostic() {
    const TICKS: u32 = 500;
    for bite_ratio in [2.0_f32, 2.5, 3.0, 3.5] {
        let mut app = make_app();
        app.insert_resource(RatioControls { bite_ratio, ..Default::default() });
        let (mut e_sum, mut e_ticks, mut max_col) = (0.0_f32, 0u32, 0usize);
        for _ in 0..TICKS {
            app.update();
            let n = elk_count(app.world_mut());
            if n > 0 {
                e_sum += total_elk_energy(app.world()) / n as f32;
                e_ticks += 1;
            }
            max_col = max_col.max(max_col_reached(app.world_mut()));
        }
        let pop = elk_count(app.world_mut());
        let mean_e = if e_ticks > 0 { e_sum / e_ticks as f32 } else { 0.0 };
        println!(
            "bite_ratio={bite_ratio:.1}: mean_energy={mean_e:.3} final_pop={pop} max_col={max_col} (break-even {:.0}%)",
            100.0 / bite_ratio
        );
    }
}

const W: usize = 40;
const H: usize = 40;

/// A loose cluster of elk near the left-centre of the plain — the starting group
/// whose fate the trace records. Spaced two cells apart so the run measures the
/// decision model, not an initial crush of elk stacked onto the same few cells.
fn cluster() -> Vec<(usize, u8)> {
    let mut starts = Vec::new();
    for row in (12..28).step_by(2) {
        for col in (2..14).step_by(2) {
            starts.push((row * W + col, 0u8));
        }
    }
    starts
}

fn print_trace(name: &str, trace: &mesopotamia::sim_harness::RunTrace) {
    for (t, s) in trace.samples.iter().enumerate() {
        if t % 50 == 49 || t == 0 {
            println!(
                "[{name}] tick {:>3}: pop={:>3} energy={:.3} centroid_col={:.2} gyration={:.2} %travel={:.2} net_e={:+.4}",
                t + 1,
                s.population,
                s.mean_energy,
                s.centroid_col,
                s.radius_of_gyration,
                s.frac_traveling,
                s.net_energy,
            );
        }
    }
}

#[test]
fn herd_on_abundant_plain_feeds_and_survives() {
    const TICKS: u32 = 400;
    let starts = cluster();
    let n0 = starts.len();

    // A plain stocked to full capacity — settle-able grass exists everywhere, so
    // a working model has no excuse to starve.
    let grid = open_plain(W, H, 1.0);
    let trace = diagnose(grid, &starts, ElkParams::default(), 0.8, TICKS);
    print_trace("rich", &trace);

    let last = trace.samples.last().expect("ran at least one tick");
    // Most of the herd survives; the run is only reproducible to an envelope (grass
    // growth and death ordering aren't seed-locked), so the floor is loose — the
    // starvation bug would leave almost none. Energy stays healthy, the signal that
    // actually separates feeding from the starvation trap.
    assert!(
        last.population >= n0 / 2,
        "herd starved on abundant grass: {} of {n0} survived to tick {TICKS}",
        last.population
    );
    assert!(
        last.mean_energy > 0.2,
        "survivors are starving on abundant grass: mean energy {:.3} at tick {TICKS}",
        last.mean_energy
    );
}

// Diagnostic (not yet a gate): a plain stocked *below* settle_frac. If a
// travelling elk can never find a cell rich enough to settle on, the mode
// deadlocks — graze suppressed, herd clumps, starves. This prints the trace so
// we can read whether that is the clump-and-die the demo shows.
#[test]
fn herd_on_thin_plain_diagnostic() {
    const TICKS: u32 = 400;
    let starts = cluster();
    // 0.5 < settle_frac (0.6): no cell is settle-able until grass regrows past it.
    let grid = open_plain(W, H, 0.5);
    let trace = diagnose(grid, &starts, ElkParams::default(), 0.8, TICKS);
    print_trace("thin", &trace);
}

// Regression gate for the travel-mode starvation trap. On the real worldgen map
// the herd must feed, survive, and migrate forward — not clump in permanent
// travel and starve. The three guards each pin a face of that bug: energy stayed
// healthy (it crashed to 0), travel was not pinned on (it was stuck at 1.0), and
// the herd advanced across the map (it was stuck near spawn). Thresholds are
// loose because make_app uses an unseeded RNG, but the bug missed them by a mile.
#[test]
fn worldgen_herd_does_not_starve_in_permanent_travel() {
    const TICKS: u32 = 800;
    let trace = diagnose_worldgen(ElkParams::default(), TICKS);
    print_trace("world", &trace);

    // Average over ticks with a live herd, so empty pre-spawn / inter-wave ticks
    // don't drag the means toward zero.
    let live: Vec<_> = trace.samples.iter().filter(|s| s.population > 0).collect();
    assert!(!live.is_empty(), "no elk ever lived");
    let mean_energy = live.iter().map(|s| s.mean_energy).sum::<f32>() / live.len() as f32;
    let mean_travel = live.iter().map(|s| s.frac_traveling).sum::<f32>() / live.len() as f32;

    // The travel-mode starvation bug had two signatures: every elk pinned in
    // travel (graze suppressed) and energy bled to ~0. Both must stay clear. The
    // floors are loose — the tightened economy legitimately runs the herd mid-fed
    // (~0.2–0.5), so this guards against total collapse, not against difficulty.
    // (Forward migration is pinned separately by `tuned_weights_cross_much_further`.)
    assert!(
        mean_energy > 0.12,
        "herd is starving out: mean live energy {mean_energy:.3} (the bug drove this to ~0)"
    );
    assert!(
        mean_travel < 0.85,
        "herd is stuck in permanent travel: mean %travel {mean_travel:.2} (the bug pinned this at 1.0)"
    );
}

// Green-wave purpose diagnostic on the real worldgen map: with migration pull
// zeroed, a wave-on run should carry the centroid farther east than wave-off.
// Not a hard gate — emergent/seed-noisy. Run with --nocapture to read the
// centroid trajectories and tune `GreenWave` defaults.
#[test]
#[ignore = "green wave crosses diagnostic — run manually: cargo test --test herd_shape green_wave_crosses -- --ignored --nocapture"]
fn green_wave_crosses_on_worldgen() {
    const TICKS: u32 = 600;
    let run = |name: &str, strength: f32| -> f32 {
        let mut app = make_app();
        app.insert_resource(RatioControls { cross_ratio: 0.0, ..Default::default() });
        app.insert_resource(GreenWave { strength, ..Default::default() });
        let mut final_centroid = 0.0f32;
        for t in 0..TICKS {
            app.update();
            let col = centroid_col(app.world_mut());
            final_centroid = col;
            if t % 99 == 98 || t == 0 {
                println!("[worldgen/{name}] tick {:>4}: centroid_col={col:.1}", t + 1);
            }
        }
        final_centroid
    };
    let off = run("wave-off", 0.0);
    let on = run("wave-on", 0.5);
    println!("worldgen: wave-off final={off:.1}, wave-on final={on:.1}, advantage={:.1}", on - off);
}

// Green-wave purpose diagnostic on a clean open plain (no shrubs).
// Shrubs can pin the herd; this isolates the grass-wave signal alone.
// Both variants seed the same plain and the same cluster for fair comparison.
#[test]
#[ignore = "green wave plain diagnostic — run manually: cargo test --test herd_shape green_wave_crosses -- --ignored --nocapture"]
fn green_wave_crosses_on_plain() {
    const TICKS: u32 = 600;
    let run = |name: &str, strength: f32| -> f32 {
        let starts = cluster();
        let grid = open_plain(W, H, 1.0);
        let mut app = make_probe_app(grid, &starts);
        app.insert_resource(RatioControls { cross_ratio: 0.0, ..Default::default() });
        app.insert_resource(GreenWave { strength, ..Default::default() });
        let mut final_centroid = 0.0f32;
        for t in 0..TICKS {
            app.update();
            let col = centroid_col(app.world_mut());
            final_centroid = col;
            if t % 99 == 98 || t == 0 {
                println!("[plain/{name}] tick {:>4}: centroid_col={col:.1}", t + 1);
            }
        }
        final_centroid
    };
    let off = run("wave-off", 0.0);
    let on = run("wave-on", 0.5);
    println!("plain: wave-off final={off:.1}, wave-on final={on:.1}, advantage={:.1}", on - off);
}
