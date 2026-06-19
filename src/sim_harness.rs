use bevy::app::ScheduleRunnerPlugin;
use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use bevy::time::TimeUpdateStrategy;
use std::time::Duration;

use crate::elk::{Decision, DriveSamples, Elk, ElkParams, ElkSimPlugin, EnergyFlows, Herds, LastDecision, ProbeSeed, Spawner};
use crate::elk::{combine_drives, grass_gradient, step_water_penalty, StepEval};
use crate::droppings::DroppingsPlugin;
use crate::grid::{Grid, GridPlugin};
use crate::sim::{Sim, SimStatePlugin};
use crate::worldgen::WorldgenPlugin;

const HZ: f64 = 10.0;
const PERIOD: Duration = Duration::from_millis(100);

/// Build a headless app without a renderer or window.
pub fn make_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins.set(ScheduleRunnerPlugin::run_once()))
        .insert_resource(Time::<Fixed>::from_hz(HZ))
        .insert_resource(TimeUpdateStrategy::ManualDuration(PERIOD))
        .add_plugins(StatesPlugin)
        .add_plugins((GridPlugin, DroppingsPlugin, ElkSimPlugin, WorldgenPlugin, SimStatePlugin));
    app
}

pub fn spawner_elapsed(world: &World) -> u32 {
    world.get_resource::<Spawner>().unwrap().elapsed
}

/// Sum of `energy_sum` across all cohorts — total energy held by live elk.
/// Valid after `tally_herds` has run (i.e., after each `app.update()`).
pub fn total_elk_energy(world: &World) -> f32 {
    world.get_resource::<Herds>().unwrap().cohorts.values().map(|c| c.energy_sum).sum()
}

/// Returns a copy of the accumulated per-tick energy flows.
/// Call after `app.update()` and before the next tick's reset to get this tick's flows.
pub fn elk_flows(world: &World) -> EnergyFlows {
    *world.get_resource::<EnergyFlows>().unwrap()
}

pub fn elk_count(world: &mut World) -> usize {
    let mut q = world.query::<&Elk>();
    q.iter(world).count()
}

pub fn max_col_reached(world: &mut World) -> usize {
    let grid_width = world.get_resource::<Grid>().unwrap().width();
    let mut q = world.query::<&Elk>();
    q.iter(world).map(|elk| elk.cell % grid_width).max().unwrap_or(0)
}

/// Per-run outcome of a headless simulation, defined by doc02.02.
pub struct RunMetrics {
    pub survival: f32,
    pub max_col: usize,
    pub mean_migration_share: f32,
}

/// Run a headless app for `ticks` steps with the given `ElkParams` and collect
/// the doc02.02 metrics: survival, farthest column reached, and time-averaged
/// migration share across active pack slots.
pub fn run_metrics(params: ElkParams, ticks: u32) -> RunMetrics {
    let mut app = make_app();
    // Override the plugin default — insert_resource replaces init_resource's value.
    app.insert_resource(params);

    let mut share_acc = 0.0_f32;
    let mut share_ticks = 0u32;
    let mut max_col = 0usize;

    for _ in 0..ticks {
        app.update();
        let world = app.world_mut();

        // Sample per-slot migration share for this tick (borrow dropped before query below).
        let tick_share: f32 = {
            let samples = world.get_resource::<DriveSamples>().unwrap();
            let active: Vec<f32> = samples
                .per_slot
                .iter()
                .filter(|s| s.count > 0)
                .map(|s| s.migration_share())
                .collect();
            if active.is_empty() { 0.0 } else { active.iter().sum::<f32>() / active.len() as f32 }
        };
        share_acc += tick_share;
        share_ticks += 1;

        let col = max_col_reached(world);
        if col > max_col {
            max_col = col;
        }
    }

    let mean_migration_share =
        if share_ticks > 0 { share_acc / share_ticks as f32 } else { 0.0 };

    let world = app.world_mut();
    let herds = world.get_resource::<Herds>().unwrap();
    let (total_deaths, total_spawned) =
        herds.cohorts.values().fold((0u32, 0u32), |(d, s), c| {
            (d + c.deaths, s + c.alive + c.deaths + c.departures)
        });
    let survival = if total_spawned > 0 {
        1.0 - total_deaths as f32 / total_spawned as f32
    } else {
        1.0
    };

    RunMetrics { survival, max_col, mean_migration_share }
}

/// doc02.02 counterfactual: run with `migration = 0` and return the farthest
/// column the herd reached — natural drives alone carry the journey.
pub fn journey_natural(params: ElkParams, ticks: u32) -> usize {
    let mut p = params;
    p.migration = 0.0;
    run_metrics(p, ticks).max_col
}

// ── Probe-world infrastructure (doc02.03, rung 6) ─────────────────────────────

/// Layout constants for the canonical crossing probe grid.
pub const PROBE_W: usize = 20;
pub const PROBE_H: usize = 11;
/// Column occupied by the water barrier.
pub const PROBE_WATER_COL: usize = 5;
/// Row where the single ford cell sits.
pub const PROBE_FORD_ROW: usize = 5;
/// Column where the elk starts on the near bank.
pub const PROBE_START_COL: usize = 2;
/// First column of the far-bank forage strip.
pub const PROBE_FAR_BANK_COL: usize = PROBE_WATER_COL + 1;

/// Build the canonical crossing-probe grid:
/// near bank (cols 0-4), one water column with a single ford, far bank with
/// full-capacity grass. Small and deterministic — no worldgen noise.
pub fn probe_grid() -> Grid {
    let mut grid = Grid::new(PROBE_W, PROBE_H);

    // Water barrier: entire PROBE_WATER_COL column is deep water.
    for row in 0..PROBE_H {
        let cell = row * PROBE_W + PROBE_WATER_COL;
        grid.set_water(cell, 1.0);
        // Zero water_prox so grass can't grow here; soil stays at default 1.0.
        grid.set_water_prox(cell, 0.0);
        grid.set_ford(cell, row == PROBE_FORD_ROW);
    }

    // Far bank: full grass capacity on all cells past the barrier.
    // Grid::new defaults water_prox=1.0 and soil=1.0, so capacity=1.0 there.
    for row in 0..PROBE_H {
        for col in PROBE_FAR_BANK_COL..PROBE_W {
            let cell = row * PROBE_W + col;
            grid.set_grass(cell, 1.0);
        }
    }

    grid
}

/// Build a headless probe app over a hand-constructed grid, bypassing worldgen.
/// Inserts `ProbeSeed(42)` so `herd_move` is deterministic across all probe runs.
/// `elk_starts` is a list of `(cell, slot)` pairs — one Elk entity is spawned per entry.
///
/// Internally runs one warm-up `update()` to process the state transition from
/// `Sim::Generating` (SimStatePlugin default) → `Sim::Running` before elk are
/// spawned. The warm-up tick consumes no elk energy (no elk in the world yet).
pub fn make_probe_app(grid: Grid, elk_starts: &[(usize, u8)]) -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins.set(ScheduleRunnerPlugin::run_once()))
        .insert_resource(Time::<Fixed>::from_hz(HZ))
        .insert_resource(TimeUpdateStrategy::ManualDuration(PERIOD))
        .add_plugins(StatesPlugin)
        .add_plugins((GridPlugin, DroppingsPlugin, ElkSimPlugin, SimStatePlugin))
        // Override the GridPlugin's default with the probe grid.
        .insert_resource(grid)
        // Freeze the spawner so spawn_waves never fires (cooldown stays maxed).
        .insert_resource(Spawner { cooldown: u32::MAX / 2, next_pack: 0, elapsed: 0 })
        // Persistent seeded RNG for deterministic probe assertions.
        .insert_resource(ProbeSeed::new(42));

    // Request transition to Running. SimStatePlugin starts in Sim::Generating;
    // run one warm-up update so StateTransition processes the transition before
    // any elk are in the world.
    app.world_mut()
        .resource_mut::<NextState<Sim>>()
        .set(Sim::Running);
    app.update(); // warm-up: Generating → Running, no elk yet

    // Spawn probe elk directly into the world (no Sprite/Transform needed —
    // movement and metabolism systems only query Elk + LastDecision).
    for &(cell, slot) in elk_starts {
        app.world_mut().spawn((
            Elk {
                cell,
                prev_cell: cell,
                slot,
                code: 0,
                energy: 0.3,
                digesting: Vec::new(),
                grazing: false,
                at_edge: 0,
                intake_rate: 0.0,
            },
            LastDecision(Decision::default()),
        ));
    }

    app
}

/// A thin trait for stepping one decision without the full ECS context.
/// Backed by `FieldDecider` which uses the real grass-gradient path, scoped
/// to field drives only (no per-elk neighbour context available at this boundary).
pub trait Decider {
    fn decide(&self, cell: usize, grid: &Grid, params: &ElkParams) -> Decision;
}

/// Field-only implementation: uses `grass_gradient` + `combine_drives` with
/// zero sep/coh/social and fixed neutral energy. Deterministic (no RNG).
///
/// **Limitation**: separation, cohesion, and social foraging are absent — these
/// require the positions of other elk, which are not available at the trait
/// boundary. For a single-agent probe this is exact; for multi-agent probes the
/// field decision is an approximation.
pub struct FieldDecider {
    /// Elk energy level fed to `combine_drives`; affects the appetite multiplier.
    pub energy: f32,
    /// Pack migration pressure (0..=1).
    pub pressure: f32,
}

impl Default for FieldDecider {
    fn default() -> Self {
        Self { energy: 0.5, pressure: 0.0 }
    }
}

impl Decider for FieldDecider {
    fn decide(&self, cell: usize, grid: &Grid, params: &ElkParams) -> Decision {
        let grass_dir = grass_gradient(cell, grid, params);
        let drives = combine_drives(
            Vec2::ZERO, Vec2::ZERO, grass_dir, Vec2::ZERO,
            params, self.pressure, self.energy, 1.0,
        );
        let desire = drives.total();

        let steps: [(isize, isize); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];
        let mut options = Vec::with_capacity(4);
        let mut best_score = f32::NEG_INFINITY;
        let mut best_idx: Option<usize> = None;

        for &(dx, dy) in &steps {
            if let Some(next) = grid.step(cell, dx, dy) {
                let penalty = step_water_penalty(
                    grid.water(next), grid.is_ford(next),
                    params.water_cost, params.ford_discount,
                );
                let score = desire.dot(Vec2::new(dx as f32, dy as f32)) - penalty;
                if score > best_score {
                    best_score = score;
                    best_idx = Some(options.len());
                }
                options.push(StepEval { step: (dx, dy), score, penalty, weight: 1.0 });
            }
        }

        let chosen = if best_score.is_finite() { best_idx } else { None };
        Decision { drives, options, chosen, temperature: params.temperature }
    }
}

/// Run the crossing probe with one drive modified and return the tick at which
/// the elk first reaches `PROBE_FAR_BANK_COL`, or `max_ticks` if it never does.
/// The delta between baseline (no-op modifier) and ablated results pins which
/// drive gates the crossing.
pub fn probe_ablation(
    mut params: ElkParams,
    zero_drive: impl Fn(&mut ElkParams),
    max_ticks: u32,
) -> u32 {
    zero_drive(&mut params);
    let start = PROBE_FORD_ROW * PROBE_W + PROBE_START_COL;
    let mut app = make_probe_app(probe_grid(), &[(start, 0)]);
    app.insert_resource(params);
    for tick in 0..max_ticks {
        app.update();
        if max_col_reached(app.world_mut()) >= PROBE_FAR_BANK_COL {
            return tick + 1;
        }
    }
    max_ticks
}
