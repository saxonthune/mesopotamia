/// Headless balancing parameter sweep (doc02.02).
///
/// Grids over the `behaviour_tab` slider ranges for the weights most relevant to
/// the balance objective — `migration`, `quiet`, and a metabolism param —
/// computes `survival`, `mean_migration_share`, and `journey_natural` for each
/// candidate, then prints rows ranked by the balance objective.
///
/// Not in `cargo test` — this is a search tool, not a correctness gate.
/// Run with: `cargo run --bin sweep`
use mesopotamia::elk::ElkParams;
use mesopotamia::sim_harness::{journey_natural, run_metrics};

// Slider ranges from behaviour_tab in ui.rs.
// Coarse grid: 4 values per key param, default for the rest.
const MIGRATION_VALUES: &[f32] = &[0.2, 0.5, 0.9, 1.5];
const QUIET_VALUES: &[f32] = &[0.3, 1.0, 2.0, 3.0];
const ENERGY_DRAIN_VALUES: &[f32] = &[0.002, 0.004, 0.006];

// doc02.02 loose thresholds — a candidate is "balanced" when it clears all three.
const S_FLOOR: f32 = 0.5;
const EDGE_BAND: usize = 200; // ~78 % of GRID_WIDTH=256
const SIGMA_MAX: f32 = 0.5;

// Ticks per candidate run — short enough to sweep fast, long enough to see dynamics.
const TICKS: u32 = 300;

fn main() {
    let total = MIGRATION_VALUES.len() * QUIET_VALUES.len() * ENERGY_DRAIN_VALUES.len();
    println!(
        "sweep: {} candidates × 2 runs (+ counterfactual) = {} simulation runs, {} ticks each",
        total,
        total * 2,
        TICKS
    );
    println!(
        "thresholds: survival≥{S_FLOOR}  journey_nat≥{EDGE_BAND}  share≤{SIGMA_MAX}\n"
    );
    println!(
        "{:<10} {:<7} {:<12}  {:<8} {:<8} {:<8}  {}",
        "migration", "quiet", "energy_drain", "survival", "max_col", "share", "balanced"
    );
    println!("{}", "-".repeat(72));

    let mut rows: Vec<Row> = Vec::with_capacity(total);

    for &mig in MIGRATION_VALUES {
        for &quiet in QUIET_VALUES {
            for &drain in ENERGY_DRAIN_VALUES {
                let mut params = ElkParams::default();
                params.migration = mig;
                params.quiet = quiet;
                params.energy_drain = drain;

                let metrics = run_metrics(params.clone(), TICKS);
                let nat = journey_natural(params.clone(), TICKS);

                let balanced = metrics.survival >= S_FLOOR
                    && nat >= EDGE_BAND
                    && metrics.mean_migration_share <= SIGMA_MAX;

                rows.push(Row {
                    migration: mig,
                    quiet,
                    energy_drain: drain,
                    survival: metrics.survival,
                    max_col_natural: nat,
                    mean_share: metrics.mean_migration_share,
                    balanced,
                });

                // Live progress so the operator can see work is proceeding.
                eprint!(
                    "\r  done {}/{} …",
                    rows.len(),
                    total
                );
            }
        }
    }
    eprintln!();

    // Rank: balanced first, then survival↑ minus share↓.
    rows.sort_by(|a, b| {
        b.balanced
            .cmp(&a.balanced)
            .then(b.score().partial_cmp(&a.score()).unwrap())
    });

    let balanced_count = rows.iter().filter(|r| r.balanced).count();

    for r in &rows {
        println!(
            "{:<10.3} {:<7.3} {:<12.4}  {:<8.3} {:<8} {:<8.3}  {}",
            r.migration,
            r.quiet,
            r.energy_drain,
            r.survival,
            r.max_col_natural,
            r.mean_share,
            if r.balanced { "YES" } else { "no" }
        );
    }

    println!("\n{balanced_count}/{total} candidates are balanced.");
    if balanced_count == 0 {
        println!("No balanced candidates found at these thresholds — widen the grid or loosen thresholds.");
    }
}

struct Row {
    migration: f32,
    quiet: f32,
    energy_drain: f32,
    survival: f32,
    max_col_natural: usize,
    mean_share: f32,
    balanced: bool,
}

impl Row {
    fn score(&self) -> f32 {
        self.survival - self.mean_share
    }
}
