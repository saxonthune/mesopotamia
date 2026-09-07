use bevy::prelude::*;

use crate::diagnostics;
use crate::elk::abundance::{self, AbundanceParams};
use crate::elk::{Elk, EnergyFlows, Herding, Herds, ENERGY_DRAIN, PACK_COUNT};
use crate::grid::{Grid, GrowthRate};
use crate::sim::Sim;

pub struct Metric {
    pub name: &'static str,
    pub extract: fn(&Elk) -> f32,
}

pub const ELK_METRICS: &[Metric] = &[
    Metric { name: "energy", extract: |e| e.energy },
];

/// Collection level for a metric — the "what to collect" dimension, like a log level. A profile
/// picks a ceiling and every metric at or below it is collected. `Core` is cheap headline state;
/// `Heavy` includes the per-slot radius scans that a demo need not pay for every tick.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum MetricTier {
    Core,
    Diagnostic,
    Heavy,
}

/// One scalar reading off the whole world — the unit of the raw data source. A fn-pointer
/// table (not a trait) so the registry is `const` and adding a stat is a one-line data edit.
/// `tier` is the declaration; the live `MetricProfile` is the policy that selects on it.
#[derive(Clone, Copy)]
pub struct WorldMetric {
    pub name: &'static str,
    pub tier: MetricTier,
    pub sample: fn(&mut World) -> f32,
}

fn elk_cells(world: &mut World) -> Vec<usize> {
    world.query::<&Elk>().iter(world).map(|e| e.cell).collect()
}

/// The default panel of world metrics every projection reads. Extend by appending a row.
pub const WORLD_METRICS: &[WorldMetric] = &[
    WorldMetric { name: "population", tier: MetricTier::Core, sample: |w| w.query::<&Elk>().iter(w).count() as f32 },
    WorldMetric {
        name: "mean_energy",
        tier: MetricTier::Core,
        sample: |w| {
            let es: Vec<f32> = w.query::<&Elk>().iter(w).map(|e| e.energy).collect();
            if es.is_empty() { 0.0 } else { es.iter().sum::<f32>() / es.len() as f32 }
        },
    },
    WorldMetric { name: "grass_mass", tier: MetricTier::Core, sample: |w| w.resource::<Grid>().total_grass() },
    WorldMetric { name: "shrub_mass", tier: MetricTier::Core, sample: |w| w.resource::<Grid>().total_shrubs() },
    // Fraction of the herd holding-and-feeding (HerdState::Graze) vs in motion. The headline
    // behavioural signal: chronically low = "won't stop to eat"; its std/mean_crossings over the
    // series surface the "settle all-at-once, then all move" oscillation.
    WorldMetric {
        name: "frac_grazing",
        tier: MetricTier::Core,
        sample: |w| {
            let states: Vec<bool> = w.query::<&Herding>().iter(w).map(|h| !h.is_traveling()).collect();
            if states.is_empty() { 0.0 } else { states.iter().filter(|g| **g).count() as f32 / states.len() as f32 }
        },
    },
    WorldMetric {
        name: "centroid_col",
        tier: MetricTier::Diagnostic,
        sample: |w| {
            let cells = elk_cells(w);
            diagnostics::centroid(&cells, w.resource::<Grid>().width()).0
        },
    },
    WorldMetric {
        name: "centroid_row",
        tier: MetricTier::Diagnostic,
        sample: |w| {
            let cells = elk_cells(w);
            diagnostics::centroid(&cells, w.resource::<Grid>().width()).1
        },
    },
    WorldMetric {
        name: "gyration",
        tier: MetricTier::Diagnostic,
        sample: |w| {
            let cells = elk_cells(w);
            diagnostics::radius_of_gyration(&cells, w.resource::<Grid>().width())
        },
    },
    // Cumulative herd energy intake (EnergyFlows is not reset per tick); the per-tick feeding
    // rate is this column's slope — flat ⇒ the herd isn't actually eating.
    WorldMetric { name: "intake_total", tier: MetricTier::Diagnostic, sample: |w| w.resource::<EnergyFlows>().intake },
    WorldMetric {
        name: "deaths",
        tier: MetricTier::Diagnostic,
        sample: |w| w.resource::<Herds>().cohorts.values().map(|c| c.deaths).sum::<u32>() as f32,
    },
    WorldMetric {
        name: "departures",
        tier: MetricTier::Diagnostic,
        sample: |w| w.resource::<Herds>().cohorts.values().map(|c| c.departures).sum::<u32>() as f32,
    },
    // View-only forage signals (nothing in the sim reads these): slider-weighted nearby
    // grass+energy per elk, and nearby-regrowth-per-elk ÷ drain (>1 ⇒ patch outpaces grazing → herd camps).
    WorldMetric { name: "abundance_per_elk", tier: MetricTier::Heavy, sample: |w| herd_means(w).0 },
    WorldMetric { name: "regrowth_drain_ratio", tier: MetricTier::Heavy, sample: |w| herd_means(w).1 },
];

/// Herd-mean forage abundance and regrowth÷drain ratio, computed per cohort slot at the slot's
/// centroid. Glue for the two view-only metrics above; the arithmetic lives (tested) in `abundance`.
fn herd_means(world: &mut World) -> (f32, f32) {
    let elk: Vec<(usize, usize, f32)> = world
        .query::<&Elk>()
        .iter(world)
        .map(|e| (e.slot as usize, e.cell, e.energy))
        .collect();

    let mut sum_col = [0.0_f32; PACK_COUNT];
    let mut sum_row = [0.0_f32; PACK_COUNT];
    let mut energy_by_slot = [0.0_f32; PACK_COUNT];
    let mut count = [0_u32; PACK_COUNT];

    let grid = world.resource::<Grid>();
    for &(slot, cell, energy) in &elk {
        if slot < PACK_COUNT {
            let (col, row) = grid.col_row(cell);
            sum_col[slot] += col as f32;
            sum_row[slot] += row as f32;
            energy_by_slot[slot] += energy;
            count[slot] += 1;
        }
    }

    let growth = world.resource::<GrowthRate>();
    let ab = world.resource::<AbundanceParams>();
    let (mut ab_sum, mut ratio_sum, mut herds) = (0.0_f32, 0.0_f32, 0_u32);
    for slot in 0..PACK_COUNT {
        let c = count[slot];
        if c == 0 {
            continue;
        }
        let col = (sum_col[slot] / c as f32).round() as usize;
        let row = (sum_row[slot] / c as f32).round() as usize;
        let grass_nearby = abundance::grass_in_radius(grid, col, row, ab.radius);
        let regrowth_nearby = abundance::regrowth_in_radius(grid, growth, col, row, ab.radius);
        let abun = abundance::local_abundance(grass_nearby, energy_by_slot[slot], ab.energy_weight);
        ab_sum += abundance::per_capita(abun, c);
        let regrowth_pe = abundance::per_capita(regrowth_nearby, c);
        ratio_sum += abundance::regrowth_drain_ratio(regrowth_pe, ENERGY_DRAIN);
        herds += 1;
    }
    if herds > 0 {
        (ab_sum / herds as f32, ratio_sum / herds as f32)
    } else {
        (0.0, 0.0)
    }
}

/// The raw data source: a time-indexed table of named scalars. Every projection (in-app graphs,
/// console/CSV for an agent, test assertions) reads this one value — the unification lives here.
/// A `Resource` so the live app samples into it just like the headless harness builds one.
#[derive(Resource, Clone, Default)]
pub struct MetricLog {
    pub names: Vec<&'static str>,
    pub rows: Vec<Vec<f32>>,
}

impl MetricLog {
    pub fn new(metrics: &[WorldMetric]) -> Self {
        Self { names: metrics.iter().map(|m| m.name).collect(), rows: Vec::new() }
    }

    /// Append one tick's reading by running each metric against the world.
    pub fn sample(&mut self, world: &mut World, metrics: &[WorldMetric]) {
        if self.names.is_empty() {
            self.names = metrics.iter().map(|m| m.name).collect();
        }
        self.rows.push(metrics.iter().map(|m| (m.sample)(world)).collect());
    }

    /// `sample`, capped to the most recent `window` rows — for an unbounded live session.
    pub fn sample_windowed(&mut self, world: &mut World, metrics: &[WorldMetric], window: usize) {
        self.sample(world, metrics);
        if self.rows.len() > window {
            self.rows.drain(0..self.rows.len() - window);
        }
    }

    /// The latest reading as one `name value` line — the compact console projection.
    pub fn snapshot_line(&self) -> Option<String> {
        let row = self.rows.last()?;
        Some(
            self.names
                .iter()
                .zip(row)
                .map(|(n, v)| format!("{n} {v:.3}"))
                .collect::<Vec<_>>()
                .join("  "),
        )
    }

    pub fn column(&self, name: &str) -> Vec<f32> {
        match self.names.iter().position(|n| *n == name) {
            Some(i) => self.rows.iter().map(|r| r[i]).collect(),
            None => Vec::new(),
        }
    }

    /// Full series as CSV (header `tick,<names>`), for offline plotting or a diffable artifact.
    pub fn to_csv(&self) -> String {
        let mut out = String::from("tick");
        for n in &self.names {
            out.push(',');
            out.push_str(n);
        }
        out.push('\n');
        for (tick, row) in self.rows.iter().enumerate() {
            out.push_str(&tick.to_string());
            for v in row {
                out.push_str(&format!(",{v}"));
            }
            out.push('\n');
        }
        out
    }

    /// Per-metric reduction table (final / min / max / mean / slope) — the at-a-glance projection
    /// for an agent or console reader who wants the outcome, not every tick.
    pub fn summary(&self) -> String {
        use crate::behaviors::series;
        let w = self.names.iter().map(|n| n.len()).max().unwrap_or(4).max(6);
        let mut out = format!(
            "{:<w$}  {:>10}  {:>10}  {:>10}  {:>10}  {:>10}\n",
            "metric", "final", "min", "max", "mean", "slope"
        );
        for name in &self.names {
            let c = self.column(name);
            out.push_str(&format!(
                "{:<w$}  {:>10.3}  {:>10.3}  {:>10.3}  {:>10.3}  {:>10.5}\n",
                name,
                series::final_value(&c),
                series::min(&c),
                series::max(&c),
                series::mean(&c),
                series::slope(&c),
            ));
        }
        out
    }
}

// ~10 min of history at 10 Hz; one console snapshot every 10 s.
const LIVE_WINDOW: usize = 6000;
const LOG_EVERY: u32 = 100;

/// The collection *policy*, declared at the binary entry point and separate from the metric
/// *declarations* in `WORLD_METRICS`: which tier to collect, how often, how much history to
/// retain, and how often to echo a console snapshot. Tests run `full()` (everything, every tick,
/// unbounded, silent); the demo runs `live()` (windowed, periodic log) and can dial `tier`/`every`
/// down to collect less. Mirrors a log level + an exporter interval.
#[derive(Resource, Clone)]
pub struct MetricProfile {
    pub tier: MetricTier,
    /// Sample every Nth tick (1 = every tick).
    pub every: u32,
    /// Retain only the last N rows; `None` = unbounded (batch/test runs).
    pub window: Option<usize>,
    /// Echo a one-line snapshot every Nth tick; `None` = silent.
    pub log_every: Option<u32>,
}

impl MetricProfile {
    /// Demo default: full detail, windowed to ~10 min, a console snapshot every 10 s.
    pub fn live() -> Self {
        Self { tier: MetricTier::Heavy, every: 1, window: Some(LIVE_WINDOW), log_every: Some(LOG_EVERY) }
    }
    /// Headless/test: everything, every tick, full history, no console noise.
    pub fn full() -> Self {
        Self { tier: MetricTier::Heavy, every: 1, window: None, log_every: None }
    }
}

impl Default for MetricProfile {
    fn default() -> Self {
        Self::live()
    }
}

#[derive(Resource, Default)]
struct LogClock(u32);

/// Runs the `WORLD_METRICS` source inside the live app under a `MetricProfile`: samples the
/// selected tier into a `MetricLog` at the profile's cadence and echoes a periodic snapshot —
/// the live counterpart to the headless `sim_harness::run`.
#[derive(Default)]
pub struct InstrumentPlugin {
    pub profile: MetricProfile,
}

impl InstrumentPlugin {
    pub fn new(profile: MetricProfile) -> Self {
        Self { profile }
    }
}

impl Plugin for InstrumentPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MetricLog>()
            .init_resource::<LogClock>()
            .insert_resource(self.profile.clone())
            .add_systems(FixedUpdate, sample_into_log.run_if(in_state(Sim::Running)));
    }
}

fn sample_into_log(world: &mut World) {
    let profile = world.resource::<MetricProfile>().clone();
    let ticks = {
        let mut clock = world.resource_mut::<LogClock>();
        clock.0 += 1;
        clock.0
    };
    if ticks % profile.every != 0 {
        return;
    }
    let active: Vec<WorldMetric> =
        WORLD_METRICS.iter().filter(|m| m.tier <= profile.tier).copied().collect();
    world.resource_scope(|world, mut log: Mut<MetricLog>| match profile.window {
        Some(w) => log.sample_windowed(world, &active, w),
        None => log.sample(world, &active),
    });
    if let Some(n) = profile.log_every {
        if ticks % n == 0 {
            if let Some(line) = world.resource::<MetricLog>().snapshot_line() {
                info!("metrics @ {ticks} ticks: {line}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_elk(energy: f32) -> Elk {
        Elk {
            cell: 0,
            prev_cell: 0,
            move_t: 1.0,
            move_rate: 0.0,
            slot: 0,
            code: 0,
            energy,
            digesting: vec![],
            grazing: false,
            at_edge: 0,
        }
    }

    #[test]
    fn energy_extractor_returns_elk_energy() {
        let elk = make_elk(0.75);
        let m = ELK_METRICS.iter().find(|m| m.name == "energy").unwrap();
        assert_eq!((m.extract)(&elk), 0.75);
    }

    #[test]
    fn energy_extractor_zero() {
        let elk = make_elk(0.0);
        let m = ELK_METRICS.iter().find(|m| m.name == "energy").unwrap();
        assert_eq!((m.extract)(&elk), 0.0);
    }

    fn log() -> MetricLog {
        MetricLog {
            names: vec!["population", "mean_energy"],
            rows: vec![vec![10.0, 0.5], vec![8.0, 0.4], vec![6.0, 0.3]],
        }
    }

    #[test]
    fn column_pulls_one_metric_across_ticks() {
        assert_eq!(log().column("population"), vec![10.0, 8.0, 6.0]);
        assert_eq!(log().column("mean_energy"), vec![0.5, 0.4, 0.3]);
        assert!(log().column("absent").is_empty());
    }

    #[test]
    fn csv_has_header_and_indexed_rows() {
        let csv = log().to_csv();
        let mut lines = csv.lines();
        assert_eq!(lines.next().unwrap(), "tick,population,mean_energy");
        assert_eq!(lines.next().unwrap(), "0,10,0.5");
        assert_eq!(lines.next().unwrap(), "1,8,0.4");
    }

    #[test]
    fn snapshot_line_formats_the_last_row() {
        let s = log().snapshot_line().unwrap();
        assert!(s.contains("population 6.000"), "snapshot shows the latest population: {s}");
        assert!(s.contains("mean_energy 0.300"), "snapshot shows the latest energy: {s}");
    }

    #[test]
    fn summary_lists_every_metric_with_a_slope() {
        let s = log().summary();
        assert!(s.contains("population"), "summary names each metric");
        assert!(s.contains("mean_energy"));
        assert!(s.lines().count() == 3, "header + one row per metric");
    }
}
