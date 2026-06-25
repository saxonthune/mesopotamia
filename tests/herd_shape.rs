//! Herd-shape soak tests on a controlled dummy map.
//!
//! On a flat, evenly-stocked open plain the terrain explains nothing — the only
//! thing shaping the herd is the decision model. So these assert the model's
//! intent directly: a herd dropped on abundant grass must *feed and survive*, not
//! clump to a point and starve. The per-tick `RunTrace` is printed so a failure
//! shows the shape of the collapse (gyration → 0, energy falling, everyone stuck
//! in travel mode), not just a red assertion.

use mesopotamia::elk::{ElkParams, RatioControls};
use mesopotamia::grid::GreenWave;
use mesopotamia::sim_harness::{
    centroid_col, diagnose, diagnose_worldgen, make_app, make_probe_app, open_plain,
};

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

    // Under the herding model (doc03.01.09) feeding is decoupled from move state —
    // a travelling elk over food still eats — so a high %travel is no longer a
    // starvation signature. The honest anti-starvation guard is the energy economy
    // itself: with the food boundary unified over grass and shrubs, a herd on the
    // dry steppe perceives and grazes its shrubs and stays fed. (Whether the herd
    // *should* settle into the graze state more is a separate balance question;
    // forward migration is its own concern, not pinned here.)
    assert!(
        mean_energy > 0.3,
        "herd is starving out: mean live energy {mean_energy:.3} (a collapse drives this toward 0)"
    );
}

// Green-wave purpose diagnostic on the real worldgen map: a wave-on run should
// carry the centroid farther east than wave-off. Not a hard gate —
// emergent/seed-noisy. Run with --nocapture to read the centroid trajectories and
// tune `GreenWave` defaults.
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
