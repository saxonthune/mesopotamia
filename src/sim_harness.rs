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

const WORLDGEN_PROBE_SEED: u64 = 0x_E1C_0DE_5EED;
const SPAWN_PROBE_SEED: u64 = 0x_5A1A_D_5EED;

/// Single-threaded: multi-threaded ordering of Grid-conflicting systems diverges even seeded runs.
fn pin_schedule_order(app: &mut App) {
    app.edit_schedule(FixedUpdate, |s| {
        s.set_executor_kind(ExecutorKind::SingleThreaded);
    });
}

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

/// Valid after `tally_herds` has run (after each `app.update()`).
pub fn total_elk_energy(world: &World) -> f32 {
    world.get_resource::<Herds>().unwrap().cohorts.values().map(|c| c.energy_sum).sum()
}

/// Call after `app.update()` and before the next tick's reset for this-tick-only flows.
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

/// doc02.02 outcome metrics.
pub struct RunMetrics {
    pub survival: f32,
    pub max_col: usize,
}

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

#[derive(Clone, Copy, Debug)]
pub struct PresetOutcome {
    pub survival: f32,
    pub max_col: usize,
    pub centroid_col: f32,
    pub score_high: f32,
    pub score_current: f32,
    pub difficulty: f32,
}

/// Economy knobs (`graze_yield`/`intrinsic`/`migration`) are derived by `apply_ratios` each tick;
/// set them via `ratios`, not `params` — values on `params` are overwritten.
pub fn evaluate_bundle(
    ratios: RatioControls,
    wave: GreenWave,
    params: ElkParams,
    ticks: u32,
) -> PresetOutcome {
    evaluate_bundle_seeded(rand::random(), ratios, wave, params, ticks)
}

/// `evaluate_bundle` over a pinned seed so two configs compare on the same map (reproducibility, not bit-determinism).
pub fn evaluate_bundle_seeded(
    seed: u64,
    ratios: RatioControls,
    wave: GreenWave,
    params: ElkParams,
    ticks: u32,
) -> PresetOutcome {
    let mut app = make_app();
    // Must insert before the first update, when `generate_world` reads it.
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

pub fn journey_natural(params: ElkParams, ticks: u32) -> usize {
    run_metrics(params, ticks).max_col
}

#[derive(Clone, Copy, Debug)]
pub struct TickSample {
    pub population: usize,
    /// Monotone decline forecasts starvation.
    pub mean_energy: f32,
    /// Rising ⇒ the mass rolls forward.
    pub centroid_col: f32,
    /// Collapsing toward 0 ⇒ clumping.
    pub radius_of_gyration: f32,
    /// Pins the graze/travel balance.
    pub frac_traveling: f32,
    /// Negative ⇒ burning store faster than feeding.
    pub net_energy: f32,
}

pub struct RunTrace {
    pub samples: Vec<TickSample>,
}

fn elk_snapshot(world: &mut World) -> Vec<(usize, f32, bool)> {
    let mut q = world.query::<(&Elk, &Herding)>();
    q.iter(world).map(|(e, h)| (e.cell, e.energy, h.is_traveling())).collect()
}

/// Run a controlled headless scenario on a hand-built grid and collect per-tick observables.
/// Spawner is frozen: the only elk are those in `elk_starts`.
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
    // Real spawns enter well-fed; override the probe's 0.3 default.
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

        // EnergyFlows is cumulative (see ledger.rs); reset so each tick's net_energy is isolated.
        *world.get_resource_mut::<EnergyFlows>().unwrap() = EnergyFlows::default();
    }
    RunTrace { samples }
}

/// `diagnose` over the real worldgen map. Use when a dummy plain won't reproduce a bug.
pub fn diagnose_worldgen(params: ElkParams, ticks: u32) -> RunTrace {
    let mut app = make_app();
    app.insert_resource(params);
    // Pin both seeds: WorldSeed for terrain, spawner seed for wave sizes. Without this the gate measures noise.
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

        // EnergyFlows is cumulative (see ledger.rs); reset so each tick's net_energy is isolated.
        *world.get_resource_mut::<EnergyFlows>().unwrap() = EnergyFlows::default();
    }
    RunTrace { samples }
}

/// Uniform open plain: controlled substrate where only the decision model shapes herd behavior.
pub fn open_plain(width: usize, height: usize, grass_frac: f32) -> Grid {
    let mut grid = Grid::new(width, height);
    for i in 0..width * height {
        grid.set_grass(i, grass_frac * grid.capacity(i));
    }
    grid
}

/// East-ramping forage gradient (no wave): controlled substrate for the *move in a direction* behaviour.
pub fn ramp_plain(width: usize, height: usize, lo_frac: f32, hi_frac: f32) -> Grid {
    let mut grid = Grid::new(width, height);
    let span = (width.max(2) - 1) as f32;
    for row in 0..height {
        for col in 0..width {
            let t = col as f32 / span;
            let frac = lo_frac + (hi_frac - lo_frac) * t;
            let i = row * width + col;
            grid.set_grass(i, frac * grid.capacity(i));
        }
    }
    grid
}

/// Open plain with one water column at `river_col` and a single ford at `ford_row` — exercises Travel→Cross without worldgen noise.
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

/// ASCII frame with elk overlaid: `g`raze, `T`ravel, `X`-cross; digit/`@` for stacks; `~`/`+`/` .:#` terrain.
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

/// Migration outcome. `edge_col` is `cull`'s despawn band, so `reached_edge`/`all_departed` measure the real exit.
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

/// Run one herd over a hand-built map and report migration milestones.
/// Registers probe elk as cohort 0 so `cull`/`metabolize` tally departures and deaths.
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

/// Per-tick centroid track from `run_behavior`: a behaviour test and the demo's preset button share one source.
pub struct BehaviorTrace {
    pub centroid_col: Vec<f32>,
    pub centroid_row: Vec<f32>,
    pub mean_energy: Vec<f32>,
    /// Each surviving elk's final column — the crossing readout (`crossed_past`).
    pub final_cols: Vec<usize>,
    /// Elk that walked off the east edge — counted as crossers since the far edge sits east of any river.
    pub departures: u32,
    pub start_pop: usize,
}

impl BehaviorTrace {
    /// Denominator is `start_pop`, so deaths short of `col` count against the fraction.
    pub fn crossed_past(&self, col: usize) -> f32 {
        if self.start_pop == 0 {
            return 0.0;
        }
        let survivors_east = self.final_cols.iter().filter(|&&c| c > col).count() as u32;
        (survivors_east + self.departures) as f32 / self.start_pop as f32
    }

    pub fn final_energy(&self) -> f32 {
        *self.mean_energy.last().unwrap_or(&0.0)
    }
}

/// Run a `Preset` over a hand-built map and trace the herd centroid.
/// `HerdParams` stays at default; feed the trace to the `behaviors` kernels to assert a behaviour.
pub fn run_behavior(
    preset: &crate::elk::presets::Preset,
    grid: Grid,
    starts: &[(usize, u8)],
    start_energy: f32,
    ticks: u32,
) -> BehaviorTrace {
    let width = grid.width();
    let start_pop = starts.len();
    let mut app = make_probe_app(grid, starts);

    let mut params = ElkParams::default();
    (preset.apply_params)(&mut params);
    app.insert_resource(params);
    app.insert_resource(preset.ratios);
    app.insert_resource(preset.green_wave);
    {
        let world = app.world_mut();
        // Register the probe herd as one cohort so `cull` tallies departures
        // (off-the-east-edge crossers), which are otherwise lost to despawn.
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

    let mut centroid_col = Vec::with_capacity(ticks as usize);
    let mut centroid_row = Vec::with_capacity(ticks as usize);
    let mut mean_energy = Vec::with_capacity(ticks as usize);
    for _ in 0..ticks {
        app.update();
        let world = app.world_mut();
        let elk = elk_snapshot(world);
        let cells: Vec<usize> = elk.iter().map(|(c, _, _)| *c).collect();
        let (col, row) = crate::diagnostics::centroid(&cells, width);
        centroid_col.push(col);
        centroid_row.push(row);
        let pop = elk.len();
        mean_energy.push(if pop > 0 {
            elk.iter().map(|(_, e, _)| *e).sum::<f32>() / pop as f32
        } else {
            0.0
        });
    }

    let world = app.world_mut();
    let mut q = world.query::<&Elk>();
    let final_cols: Vec<usize> = q.iter(world).map(|e| e.cell % width).collect();
    let departures = world
        .get_resource::<Herds>()
        .unwrap()
        .cohorts
        .get(&0)
        .map_or(0, |c| c.departures);

    BehaviorTrace { centroid_col, centroid_row, mean_energy, final_cols, departures, start_pop }
}

pub const PROBE_W: usize = 20;
pub const PROBE_H: usize = 11;
pub const PROBE_WATER_COL: usize = 5;
pub const PROBE_FORD_ROW: usize = 5;
pub const PROBE_START_COL: usize = 2;
pub const PROBE_FAR_BANK_COL: usize = PROBE_WATER_COL + 1;

/// Canonical crossing-probe grid: near bank, one water column with a single ford, full-grass far bank.
pub fn probe_grid() -> Grid {
    let mut grid = Grid::new(PROBE_W, PROBE_H);

    for row in 0..PROBE_H {
        let cell = row * PROBE_W + PROBE_WATER_COL;
        grid.set_water(cell, 1.0);
        // Zero water_prox so grass can't grow here; soil stays at default 1.0.
        grid.set_water_prox(cell, 0.0);
        grid.set_ford(cell, row == PROBE_FORD_ROW);
    }

    for row in 0..PROBE_H {
        for col in PROBE_FAR_BANK_COL..PROBE_W {
            let cell = row * PROBE_W + col;
            grid.set_grass(cell, 1.0);
        }
    }

    grid
}

/// Headless probe app over a hand-built grid. Runs one warm-up tick so `Sim::Generating → Running`
/// before elk are spawned (warm-up consumes no energy; no elk yet).
pub fn make_probe_app(grid: Grid, elk_starts: &[(usize, u8)]) -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins.set(ScheduleRunnerPlugin::run_once()))
        .insert_resource(Time::<Fixed>::from_hz(HZ))
        .insert_resource(TimeUpdateStrategy::ManualDuration(PERIOD))
        .add_plugins(StatesPlugin)
        .add_plugins((GridPlugin, DroppingsPlugin, ElkSimPlugin, SimStatePlugin))
        .insert_resource(grid)
        // Freeze the spawner so spawn_waves never fires (cooldown stays maxed).
        .insert_resource(Spawner { cooldown: u32::MAX / 2, ..Default::default() });
    pin_schedule_order(&mut app);

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

/// Run the crossing probe with one drive zeroed; returns the tick the elk first reaches `PROBE_FAR_BANK_COL`.
/// Delta vs baseline (no-op) pins which drive gates the crossing.
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
