use bevy::app::ScheduleRunnerPlugin;
use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use bevy::time::TimeUpdateStrategy;
use std::time::Duration;

use crate::elk::{Decision, DriveSamples, Elk, ElkParams, ElkSimPlugin, EnergyFlows, Herds, LastDecision, ProbeSeed, RatioControls, Score, Spawner};
use crate::elk::{combine_drives, grass_gradient, graze_value, stand_value, step_water_penalty, Act, Candidate};
use crate::droppings::DroppingsPlugin;
use crate::grid::{GreenWave, Grid, GridPlugin};
use crate::sim::{Sim, SimStatePlugin};
use crate::worldgen::{WorldSeed, WorldgenPlugin};

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

/// Mean column (centroid) of all elk on the grid, or 0.0 if no elk are alive.
pub fn centroid_col(world: &mut World) -> f32 {
    let grid_width = world.get_resource::<Grid>().unwrap().width();
    let mut q = world.query::<&Elk>();
    let cells: Vec<usize> = q.iter(world).map(|e| e.cell).collect();
    if cells.is_empty() {
        return 0.0;
    }
    let (col, _) = crate::diagnostics::centroid(&cells, grid_width);
    col
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

/// Outcome of evaluating a full slider bundle over the real worldgen map — the
/// signals that define each preset's goal. `survival`/`max_col`/`centroid_col`
/// say whether the herd lived and how far east it rolled; `mean_migration_share`
/// and `pull_share` say how much of that was bought with the magic pull; the
/// `score_*` and `difficulty` fields read the live Survival Score the player sees.
#[derive(Clone, Copy, Debug)]
pub struct PresetOutcome {
    pub survival: f32,
    pub max_col: usize,
    pub centroid_col: f32,
    pub mean_migration_share: f32,
    pub score_high: f32,
    pub score_current: f32,
    pub difficulty: f32,
    pub pull_share: f32,
}

/// Run the real worldgen scenario with a full slider bundle and read out the
/// preset-defining signals. The economy knobs (`graze_yield`/`intrinsic`/
/// `migration`) are *derived* from `ratios` by `apply_ratios` every tick, so set
/// those via `ratios` — not on `params`, where they would be overwritten. `params`
/// carries the un-derived drive weights / radii / temperature; `wave` sets the
/// green wave. This is the single evaluator both the preset sweeps and the preset
/// verification tests run against.
pub fn evaluate_bundle(
    ratios: RatioControls,
    wave: GreenWave,
    params: ElkParams,
    ticks: u32,
) -> PresetOutcome {
    evaluate_bundle_seeded(rand::random(), ratios, wave, params, ticks)
}

/// `evaluate_bundle` over a *pinned* worldgen seed, so two configs can be
/// compared on the identical map (reproducibility, not bit-determinism — the
/// verification skill's envelope rule). Pin the seed and a wave-off/wave-on
/// pair differ only by the knob under test, not by which random world they drew.
pub fn evaluate_bundle_seeded(
    seed: u64,
    ratios: RatioControls,
    wave: GreenWave,
    params: ElkParams,
    ticks: u32,
) -> PresetOutcome {
    let mut app = make_app();
    // Override the WorldgenPlugin's random default before the first update
    // (when `generate_world` reads it), pinning the map for this run.
    app.insert_resource(WorldSeed(seed));
    app.insert_resource(params);
    app.insert_resource(ratios);
    app.insert_resource(wave);

    let mut share_acc = 0.0_f32;
    let mut share_ticks = 0u32;
    let mut max_col = 0usize;

    for _ in 0..ticks {
        app.update();
        let world = app.world_mut();
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
        max_col = max_col.max(max_col_reached(world));
    }

    let centroid = centroid_col(app.world_mut());
    let mean_migration_share =
        if share_ticks > 0 { share_acc / share_ticks as f32 } else { 0.0 };

    let world = app.world_mut();
    let score = world.get_resource::<Score>().unwrap();
    let (score_high, score_current, difficulty, pull_share) =
        (score.high, score.current, score.difficulty, score.pull_share);

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

    PresetOutcome {
        survival,
        max_col,
        centroid_col: centroid,
        mean_migration_share,
        score_high,
        score_current,
        difficulty,
        pull_share,
    }
}

/// doc02.02 counterfactual: run with `migration = 0` and return the farthest
/// column the herd reached — natural drives alone carry the journey.
pub fn journey_natural(params: ElkParams, ticks: u32) -> usize {
    let mut p = params;
    p.migration = 0.0;
    run_metrics(p, ticks).max_col
}

// ── Diagnostic tracing (herd-shape time series) ───────────────────────────────

/// One tick's worth of herd-shape observables, read out of a headless run.
/// Together these distinguish a clump (gyration → 0, energy falling), a particle
/// cloud (bounded gyration, flat centroid), and a rolling wave (bounded gyration,
/// advancing centroid).
#[derive(Clone, Copy, Debug)]
pub struct TickSample {
    pub population: usize,
    /// Mean elk energy in [0, 1]; a monotone decline forecasts starvation.
    pub mean_energy: f32,
    /// Centroid column — the migration signal. Rising ⇒ the mass rolls forward.
    pub centroid_col: f32,
    /// Radius of gyration (cells) — herd spread. Collapsing toward 0 ⇒ clumping.
    pub radius_of_gyration: f32,
    /// Fraction of elk in travel mode this tick — pins the graze/travel balance.
    pub frac_traveling: f32,
    /// Net energy flow this tick: intake − drain − swim. Negative ⇒ the herd is
    /// burning its store faster than it feeds, i.e. on a path to death.
    pub net_energy: f32,
}

/// A per-tick time series of herd-shape observables from one headless run.
pub struct RunTrace {
    pub samples: Vec<TickSample>,
}

/// Collect `(cell, energy, traveling)` for every elk in the world.
fn elk_snapshot(world: &mut World) -> Vec<(usize, f32, bool)> {
    let mut q = world.query::<&Elk>();
    q.iter(world).map(|e| (e.cell, e.energy, e.traveling)).collect()
}

/// Run a controlled headless scenario — a hand-authored `grid`, a fixed set of
/// `elk_starts`, and a `params` set — for `ticks` steps, sampling herd-shape
/// observables each tick. Built on `make_probe_app`, so the spawner is frozen
/// (the only elk are the ones placed here) and the RNG is seeded for replay.
/// This is the readout that says whether a slider set rolls the herd forward or
/// clumps it to death, without launching the GUI.
pub fn diagnose(
    grid: Grid,
    elk_starts: &[(usize, u8)],
    params: ElkParams,
    start_energy: f32,
    ticks: u32,
) -> RunTrace {
    let width = grid.width();
    let mut app = make_probe_app(grid, elk_starts);
    app.insert_resource(params);
    // Override the probe's fixed 0.3 spawn energy so the run reflects the model,
    // not a starting handicap — real spawns enter well-fed.
    {
        let world = app.world_mut();
        let mut q = world.query::<&mut Elk>();
        for mut elk in q.iter_mut(world) {
            elk.energy = start_energy;
        }
    }

    let mut samples = Vec::with_capacity(ticks as usize);
    for _ in 0..ticks {
        app.update();
        let world = app.world_mut();
        let flows = elk_flows(world);
        let net_energy = flows.intake - flows.drain - flows.swim;

        let elk = elk_snapshot(world);
        let population = elk.len();
        let cells: Vec<usize> = elk.iter().map(|(c, _, _)| *c).collect();
        let (centroid_col, _) = crate::diagnostics::centroid(&cells, width);
        let radius_of_gyration = crate::diagnostics::radius_of_gyration(&cells, width);
        let mean_energy = if population > 0 {
            elk.iter().map(|(_, e, _)| *e).sum::<f32>() / population as f32
        } else {
            0.0
        };
        let frac_traveling = if population > 0 {
            elk.iter().filter(|(_, _, t)| *t).count() as f32 / population as f32
        } else {
            0.0
        };

        samples.push(TickSample {
            population,
            mean_energy,
            centroid_col,
            radius_of_gyration,
            frac_traveling,
            net_energy,
        });

        // EnergyFlows accumulates until a consumer resets it (see ledger.rs);
        // zero it so next tick's `net_energy` is this-tick-only, not cumulative.
        *world.get_resource_mut::<EnergyFlows>().unwrap() = EnergyFlows::default();
    }
    RunTrace { samples }
}

/// Like `diagnose`, but over the *real* worldgen map and the real spawner — the
/// actual scenario the GUI runs. Herd-shape over all elk mixes cohorts, so read
/// `population`, `mean_energy`, and `frac_traveling` as the primary signals here;
/// they alone tell clump-and-die (energy crashing, travel pinned high) apart from
/// a healthy herd. Use this when a dummy plain won't reproduce a bug.
pub fn diagnose_worldgen(params: ElkParams, ticks: u32) -> RunTrace {
    let mut app = make_app();
    app.insert_resource(params);
    let width = app.world().get_resource::<Grid>().unwrap().width();

    let mut samples = Vec::with_capacity(ticks as usize);
    for _ in 0..ticks {
        app.update();
        let world = app.world_mut();
        let flows = elk_flows(world);
        let net_energy = flows.intake - flows.drain - flows.swim;

        let elk = elk_snapshot(world);
        let population = elk.len();
        let cells: Vec<usize> = elk.iter().map(|(c, _, _)| *c).collect();
        let (centroid_col, _) = crate::diagnostics::centroid(&cells, width);
        let radius_of_gyration = crate::diagnostics::radius_of_gyration(&cells, width);
        let mean_energy = if population > 0 {
            elk.iter().map(|(_, e, _)| *e).sum::<f32>() / population as f32
        } else {
            0.0
        };
        let frac_traveling = if population > 0 {
            elk.iter().filter(|(_, _, t)| *t).count() as f32 / population as f32
        } else {
            0.0
        };
        samples.push(TickSample {
            population,
            mean_energy,
            centroid_col,
            radius_of_gyration,
            frac_traveling,
            net_energy,
        });

        // EnergyFlows accumulates until a consumer resets it (see ledger.rs);
        // zero it so next tick's `net_energy` is this-tick-only, not cumulative.
        *world.get_resource_mut::<EnergyFlows>().unwrap() = EnergyFlows::default();
    }
    RunTrace { samples }
}

/// A dummy map with no worldgen: a uniform open plain whose every cell carries
/// grass at `grass_frac` of its capacity. The controlled substrate for isolating
/// herd dynamics from terrain — on flat, evenly-stocked ground the only thing
/// shaping the herd is the decision model, so clumping or rolling is unambiguous.
pub fn open_plain(width: usize, height: usize, grass_frac: f32) -> Grid {
    let mut grid = Grid::new(width, height);
    for i in 0..width * height {
        grid.set_grass(i, grass_frac * grid.capacity(i));
    }
    grid
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
        .insert_resource(Spawner { cooldown: u32::MAX / 2, next_pack: 0, elapsed: 0, anchor: None })
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
                traveling: false,
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
        let mut options = Vec::with_capacity(6);
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
                options.push(Candidate { act: Act::Step(dx, dy), score, penalty, weight: 1.0 });
            }
        }

        // Stand candidate
        {
            let score = stand_value();
            if score > best_score {
                best_score = score;
                best_idx = Some(options.len());
            }
            options.push(Candidate { act: Act::Stand, score, penalty: 0.0, weight: 1.0 });
        }

        // Graze candidate
        {
            let here_forage = grid.forage(cell);
            let score = graze_value(here_forage, self.energy, params.dwell);
            if score > best_score {
                best_score = score;
                best_idx = Some(options.len());
            }
            options.push(Candidate { act: Act::Graze, score, penalty: 0.0, weight: 1.0 });
        }

        let chosen = if best_score.is_finite() { best_idx } else { None };
        let chosen_act = if let Some(idx) = chosen {
            options[idx].act
        } else {
            Act::Stand
        };
        Decision { drives, options, chosen, chosen_act, temperature: params.temperature }
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
