use bevy::app::ScheduleRunnerPlugin;
use bevy::ecs::schedule::ExecutorKind;
use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use bevy::time::TimeUpdateStrategy;
use std::time::Duration;

use crate::elk::{Cohort, Elk, ElkParams, ElkSimPlugin, EnergyFlows, HerdParams, Herding, Herds, RatioControls, Score, Spawner};
use crate::droppings::DroppingsPlugin;
use crate::grid::{GreenWave, Grid, GridPlugin};
use crate::sim::{Sim, SimStatePlugin};
use crate::worldgen::{WorldSeed, WorldgenPlugin};

const HZ: f64 = 10.0;
const PERIOD: Duration = Duration::from_millis(100);

/// Fixed seeds for the reproducible worldgen diagnostic — one for the terrain,
/// one (decorrelated) for the spawn sequence — so the 800-tick gate replays a
/// single pinned run instead of fresh entropy each time.
const WORLDGEN_PROBE_SEED: u64 = 0x_E1C_0DE_5EED;
const SPAWN_PROBE_SEED: u64 = 0x_5A1A_D_5EED;

/// Pin FixedUpdate to a single-threaded executor so a headless run replays
/// identically. The multi-threaded executor leaves Grid-conflicting systems
/// (graze, growth, fertilize) in a thread-timing-dependent order; in a chaotic
/// flocking sim one early flip cascades, so even a seeded run can diverge.
fn pin_schedule_order(app: &mut App) {
    app.edit_schedule(FixedUpdate, |s| {
        s.set_executor_kind(ExecutorKind::SingleThreaded);
    });
}

/// Build a headless app without a renderer or window.
pub fn make_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins.set(ScheduleRunnerPlugin::run_once()))
        .insert_resource(Time::<Fixed>::from_hz(HZ))
        .insert_resource(TimeUpdateStrategy::ManualDuration(PERIOD))
        .add_plugins(StatesPlugin)
        .add_plugins((GridPlugin, DroppingsPlugin, ElkSimPlugin, WorldgenPlugin, SimStatePlugin));
    pin_schedule_order(&mut app);
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
}

/// Run a headless app for `ticks` steps with the given `ElkParams` and collect
/// the doc02.02 metrics: survival and farthest column reached.
pub fn run_metrics(params: ElkParams, ticks: u32) -> RunMetrics {
    let mut app = make_app();
    // Override the plugin default — insert_resource replaces init_resource's value.
    app.insert_resource(params);

    let mut max_col = 0usize;

    for _ in 0..ticks {
        app.update();
        let col = max_col_reached(app.world_mut());
        if col > max_col {
            max_col = col;
        }
    }

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

    RunMetrics { survival, max_col }
}

/// Outcome of evaluating a full slider bundle over the real worldgen map — the
/// signals that define each preset's goal. `survival`/`max_col`/`centroid_col`
/// say whether the herd lived and how far east it rolled; the `score_*` and
/// `difficulty` fields read the live Survival Score the player sees.
#[derive(Clone, Copy, Debug)]
pub struct PresetOutcome {
    pub survival: f32,
    pub max_col: usize,
    pub centroid_col: f32,
    pub score_high: f32,
    pub score_current: f32,
    pub difficulty: f32,
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

    let mut max_col = 0usize;

    for _ in 0..ticks {
        app.update();
        max_col = max_col.max(max_col_reached(app.world_mut()));
    }

    let centroid = centroid_col(app.world_mut());

    let world = app.world_mut();
    let score = world.get_resource::<Score>().unwrap();
    let (score_high, score_current, difficulty) =
        (score.high, score.current, score.difficulty);

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
        score_high,
        score_current,
        difficulty,
    }
}

/// doc02.02 counterfactual: the farthest column the herd reaches on the natural
/// forage drives. The migration *pull* is retired — the herding model only fords on
/// forage — so this is the journey by construction, with no compass to disable.
pub fn journey_natural(params: ElkParams, ticks: u32) -> usize {
    run_metrics(params, ticks).max_col
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
    let mut q = world.query::<(&Elk, &Herding)>();
    q.iter(world).map(|(e, h)| (e.cell, e.energy, h.is_traveling())).collect()
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
    // Pin both seeds so the diagnostic replays identically: WorldSeed governs the
    // terrain (make_app's default is fresh entropy), and the spawner seed governs
    // each wave's size and position. Without this the gate measures noise.
    app.insert_resource(WorldSeed(WORLDGEN_PROBE_SEED));
    app.world_mut().resource_mut::<Spawner>().reseed(SPAWN_PROBE_SEED);
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

/// A uniform open plain split by one vertical river at `river_col`, with a single
/// ford at `ford_row`. The dry-land analogue of `open_plain` plus a crossable
/// barrier — the smallest fixed feature that exercises the Travel→Cross handoff
/// without worldgen noise. Grass fills both banks to `grass_frac` of capacity; the
/// river column carries deep water (no grass) save the ford cell, which stays dry
/// land so a herd has exactly one place to cross.
pub fn river_plain(
    width: usize,
    height: usize,
    grass_frac: f32,
    river_col: usize,
    ford_row: usize,
) -> Grid {
    let mut grid = Grid::new(width, height);
    for i in 0..width * height {
        grid.set_grass(i, grass_frac * grid.capacity(i));
    }
    for row in 0..height {
        let cell = row * width + river_col;
        if row == ford_row {
            grid.set_ford(cell, true);
            continue;
        }
        grid.set_water(cell, 1.0);
        grid.set_water_prox(cell, 0.0);
        grid.set_grass(cell, 0.0);
    }
    grid
}

/// An ASCII snapshot of the grid with elk overlaid — the per-tick spatial readout
/// the aggregate traces can't give. Each cell is one character: an elk's state
/// where one stands (`g`raze, `T`ravel, `X`-cross), a digit/`@` where several
/// stack, else terrain (`~` water, `+` ford, ` .:#` for rising forage). This is
/// what makes a stuck pair or a diffusing blob visible at a glance.
pub fn ascii_frame(world: &mut World) -> String {
    use crate::elk::HerdState;
    use std::collections::HashMap;

    let mut occ: HashMap<usize, Vec<HerdState>> = HashMap::new();
    let mut q = world.query::<(&Elk, &Herding)>();
    for (e, h) in q.iter(world) {
        occ.entry(e.cell).or_default().push(h.state);
    }

    let grid = world.get_resource::<Grid>().unwrap();
    let (w, h) = (grid.width(), grid.height());
    let mut out = String::with_capacity((w + 1) * h);
    for row in 0..h {
        for col in 0..w {
            let cell = row * w + col;
            let ch = match occ.get(&cell) {
                Some(states) if states.len() == 1 => match states[0] {
                    HerdState::Graze => 'g',
                    HerdState::Travel => 'T',
                    HerdState::Cross => 'X',
                },
                Some(states) if states.len() <= 9 => {
                    char::from_digit(states.len() as u32, 10).unwrap()
                }
                Some(_) => '@',
                None if grid.is_ford(cell) => '+',
                None if grid.water(cell) > 0.01 => '~',
                None => {
                    let f = grid.food_frac(cell);
                    if f < 0.1 { ' ' } else if f < 0.4 { '.' } else if f < 0.7 { ':' } else { '#' }
                }
            };
            out.push(ch);
        }
        out.push('\n');
    }
    out
}

/// The happy-path milestones of one isolated herd run: did the mass advance east,
/// did it reach the far edge, and how many elk left the map versus died versus
/// linger. `edge_col` is the runtime far-edge band `cull` despawns at, so
/// `reached_edge` and `all_departed` measure the actual end-of-map exit, not a
/// proxy. This is the readout that says "the herd crossed and walked off the map"
/// without watching frames.
#[derive(Clone, Copy, Debug)]
pub struct MigrationReport {
    pub start_pop: usize,
    pub start_col: f32,
    pub end_col: f32,
    pub max_col: usize,
    pub edge_col: usize,
    pub alive: usize,
    pub deaths: u32,
    pub departures: u32,
}

impl MigrationReport {
    /// The herd's leading edge touched the despawn band at least once.
    pub fn reached_edge(&self) -> bool {
        self.max_col >= self.edge_col
    }
    /// Every elk that started either walked off the far edge — none died, none stuck.
    pub fn all_departed(&self) -> bool {
        self.departures as usize == self.start_pop && self.deaths == 0 && self.alive == 0
    }
}

/// Run one isolated herd over a hand-built map and report the happy-path milestones.
/// Registers the probe elk as a single cohort so `cull`/`metabolize` tally
/// departures and deaths (probe elk are otherwise cohort-less), then steps `ticks`
/// times tracking the farthest column reached. Use `ascii_frame` in a test loop to
/// *watch* a run; use this to *assert* one.
pub fn run_migration(
    grid: Grid,
    starts: &[(usize, u8)],
    params: ElkParams,
    herd: HerdParams,
    start_energy: f32,
    ticks: u32,
) -> MigrationReport {
    let edge_col = grid.width().saturating_sub(2);
    let start_pop = starts.len();
    let mut app = make_probe_app(grid, starts);
    app.insert_resource(params);
    app.insert_resource(herd);
    {
        let world = app.world_mut();
        world
            .get_resource_mut::<Herds>()
            .unwrap()
            .cohorts
            .insert(0, Cohort { spawned: start_pop as u32, ..Default::default() });
        let mut q = world.query::<&mut Elk>();
        for mut elk in q.iter_mut(world) {
            elk.energy = start_energy;
        }
    }

    let start_col = centroid_col(app.world_mut());
    let mut max_col = 0usize;
    for _ in 0..ticks {
        app.update();
        max_col = max_col.max(max_col_reached(app.world_mut()));
    }
    let end_col = centroid_col(app.world_mut());

    let world = app.world_mut();
    let c = world.get_resource::<Herds>().unwrap().cohorts.get(&0).cloned().unwrap_or_default();
    MigrationReport {
        start_pop,
        start_col,
        end_col,
        max_col,
        edge_col,
        alive: c.alive as usize,
        deaths: c.deaths,
        departures: c.departures,
    }
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
/// `elk_starts` is a list of `(cell, slot)` pairs — one Elk entity is spawned per entry.
/// The herding movement model is deterministic (no RNG), so probe runs replay exactly.
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
        .insert_resource(Spawner { cooldown: u32::MAX / 2, ..Default::default() });
    pin_schedule_order(&mut app);

    // Request transition to Running. SimStatePlugin starts in Sim::Generating;
    // run one warm-up update so StateTransition processes the transition before
    // any elk are in the world.
    app.world_mut()
        .resource_mut::<NextState<Sim>>()
        .set(Sim::Running);
    app.update(); // warm-up: Generating → Running, no elk yet

    // Spawn probe elk directly into the world (no Sprite/Transform needed —
    // movement and metabolism systems only query Elk + Herding).
    for &(cell, slot) in elk_starts {
        app.world_mut().spawn((
            Elk {
                cell,
                prev_cell: cell,
                move_t: 1.0,
                move_rate: 0.0,
                slot,
                code: 0,
                energy: 0.3,
                digesting: Vec::new(),
                grazing: false,
                at_edge: 0,
            },
            Herding::default(),
        ));
    }

    app
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
