//! Herd-shape soak tests on a flat, evenly-stocked open plain.
//! Terrain explains nothing here — any failure belongs to the decision model alone.

use mesopotamia::elk::{ElkParams, RatioControls};
use mesopotamia::grid::GreenWave;
use mesopotamia::sim_harness::{
    centroid_col, diagnose, diagnose_worldgen, make_app, make_probe_app, open_plain,
};

const W: usize = 40;
const H: usize = 40;

// spaced two cells apart so the run measures the decision model, not an initial crush
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

    let grid = open_plain(W, H, 1.0);
    let trace = diagnose(grid, &starts, ElkParams::default(), 0.8, TICKS);
    print_trace("rich", &trace);

    let last = trace.samples.last().expect("ran at least one tick");
    // floor is loose (grass/death order unseeded); the starvation bug left almost none alive
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

#[test]
fn herd_on_thin_plain_diagnostic() {
    const TICKS: u32 = 400;
    let starts = cluster();
    // 0.5 < settle_frac (0.6): no cell is settle-able until grass regrows past it
    let grid = open_plain(W, H, 0.5);
    let trace = diagnose(grid, &starts, ElkParams::default(), 0.8, TICKS);
    print_trace("thin", &trace);
}

// regression gate: travel-mode starvation trap (energy→0, %travel pinned at 1.0, herd stuck near spawn)
#[test]
fn worldgen_herd_does_not_starve_in_permanent_travel() {
    const TICKS: u32 = 800;
    let trace = diagnose_worldgen(ElkParams::default(), TICKS);
    print_trace("world", &trace);

    // skip pre-spawn / inter-wave ticks so they don't drag the mean toward zero
    let live: Vec<_> = trace.samples.iter().filter(|s| s.population > 0).collect();
    assert!(!live.is_empty(), "no elk ever lived");
    let mean_energy = live.iter().map(|s| s.mean_energy).sum::<f32>() / live.len() as f32;

    // doc03.01.09: a travelling elk still eats, so %travel is no longer a starvation signature; energy is
    assert!(
        mean_energy > 0.3,
        "herd is starving out: mean live energy {mean_energy:.3} (a collapse drives this toward 0)"
    );
}

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

// isolates grass-wave signal on a shrub-free plain (shrubs can pin the herd)
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
