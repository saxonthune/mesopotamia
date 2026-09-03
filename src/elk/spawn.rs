use bevy::prelude::*;
use rand::Rng;

use crate::events::{Event, EventKind, EventLog};
use crate::grid::{Grid, GRID_HEIGHT, GRID_WIDTH};
use crate::worldgen::testmap::WorldSource;

use super::components::{Cohort, Elk, Herds, ManualSpawn, Spawner, MAX_COHORTS};
use super::herding::Herding;
use super::ledger::EnergyFlows;
use super::color::elk_color;

const TILE_SIZE: f32 = 16.0;

pub const TARGET_POPULATION: usize = 560; // fixed headcount — does NOT scale with grid area
const MAX_POPULATION: usize = 700;
const WAVE_INTERVAL: u32 = 280;
const PACK_BASE: usize = 70;
const TARGET_GROWTH_PERIOD: u32 = 1200;
const PACK_GROWTH_PERIOD: u32 = 2400;
pub const EDGE_COL: usize = GRID_WIDTH - 2; // the two farthest columns count as "at the edge"
const EDGE_TICKS: u32 = 3;

// Anchor random-walks inside a left-edge zone; occasional jump to a fresh spot.
const SPAWN_ZONE_COLS: f32 = 6.0;
const ANCHOR_WALK_MIN: f32 = 18.0;
const ANCHOR_WALK_MAX: f32 = 40.0;
const HERD_SPREAD: f32 = 6.0;
const SIZE_JITTER: f32 = 0.5;
const JUMP_PROB: f32 = 0.2; // chance each wave to reseed the anchor at random

fn herd_size(base: usize, rng: &mut impl Rng, jitter: f32) -> usize {
    let factor = rng.random_range(1.0 - jitter..1.0 + jitter);
    ((base as f32 * factor).round() as usize).max(1)
}

fn seed_anchor(rng: &mut impl Rng, zone_cols: f32) -> Vec2 {
    Vec2::new(rng.random_range(0.0..zone_cols), (GRID_HEIGHT / 2) as f32)
}

fn random_anchor(rng: &mut impl Rng, zone_cols: f32) -> Vec2 {
    Vec2::new(
        rng.random_range(0.0..zone_cols),
        rng.random_range(0.0..GRID_HEIGHT as f32),
    )
}

/// Guaranteed vertical step in `[min_walk, max_walk]`, reflecting at grid edges.
fn step_anchor(anchor: Vec2, rng: &mut impl Rng, min_walk: f32, max_walk: f32, zone_cols: f32) -> Vec2 {
    let max_row = (GRID_HEIGHT - 1) as f32;
    let mag = rng.random_range(min_walk..max_walk);
    let dir = if rng.random_range(0.0..1.0) < 0.5 { -1.0 } else { 1.0 };
    let mut row = anchor.y + dir * mag;
    // Reflect once off each edge so the step keeps its length near a wall.
    if row < 0.0 {
        row = -row;
    } else if row > max_row {
        row = 2.0 * max_row - row;
    }
    Vec2::new(rng.random_range(0.0..zone_cols), row.clamp(0.0, max_row))
}

fn scatter(anchor: Vec2, size: usize, rng: &mut impl Rng, spread: f32) -> Vec<usize> {
    (0..size)
        .map(|_| {
            let col = (anchor.x + rng.random_range(-spread..spread))
                .round()
                .clamp(0.0, (GRID_WIDTH - 1) as f32) as usize;
            let row = (anchor.y + rng.random_range(-spread..spread))
                .round()
                .clamp(0.0, (GRID_HEIGHT - 1) as f32) as usize;
            row * GRID_WIDTH + col
        })
        .collect()
}

/// ±`RESTLESS_SPREAD` around neutral: the per-elk leave-bar spread that gives the herd a
/// front-to-back personality gradient (restless elk pull ahead, content elk trail).
const RESTLESS_SPREAD: f32 = 0.3;

fn spawn_cohort(commands: &mut Commands, slot: u8, code: u32, cells: &[usize], rng: &mut impl Rng) {
    let color = elk_color(slot as usize, false);
    for &cell in cells {
        commands.spawn((
            Sprite::from_color(color, Vec2::splat(TILE_SIZE * 0.7)),
            Transform::from_xyz(0.0, 0.0, 1.0),
            Elk {
                cell,
                prev_cell: cell,
                move_t: 1.0,
                move_rate: 0.0,
                slot,
                code,
                energy: 1.0,
                digesting: Vec::new(),
                grazing: false,
                at_edge: 0,
            },
            Herding {
                restlessness: rng.random_range(1.0 - RESTLESS_SPREAD..1.0 + RESTLESS_SPREAD),
                ..Default::default()
            },
        ));
    }
}

pub(super) fn tally_herds(mut herds: ResMut<Herds>, elk: Query<&Elk>) {
    for c in herds.cohorts.values_mut() {
        c.alive = 0;
        c.energy_sum = 0.0;
    }
    for elk in &elk {
        if let Some(c) = herds.cohorts.get_mut(&elk.code) {
            c.alive += 1;
            c.energy_sum += elk.energy;
        }
    }
    for c in herds.cohorts.values_mut() {
        c.peak = c.peak.max(c.alive);
    }
}

pub(super) fn spawn_waves(
    mut commands: Commands,
    mut spawner: ResMut<Spawner>,
    mut herds: ResMut<Herds>,
    elk: Query<(), With<Elk>>,
    mut flows: ResMut<EnergyFlows>,
) {
    spawner.elapsed += 1;
    if spawner.cooldown > 0 {
        spawner.cooldown -= 1;
        return;
    }

    let target = (TARGET_POPULATION + (spawner.elapsed / TARGET_GROWTH_PERIOD) as usize)
        .min(MAX_POPULATION);
    let base = PACK_BASE + (spawner.elapsed / PACK_GROWTH_PERIOD) as usize;

    if elk.iter().count() >= target {
        return;
    }

    // Deref once so the RNG field and the bookkeeping fields are disjoint borrows
    // (field access through `ResMut`'s Deref would borrow the whole resource).
    let spawner = &mut *spawner;
    let slot = spawner.next_pack;
    let code = spawner.rng.random_range(0..0x0100_0000u32); // 6 hex digits
    let size = herd_size(base, &mut spawner.rng, SIZE_JITTER);

    let prev_anchor = spawner.anchor;
    let anchor = match prev_anchor {
        None => seed_anchor(&mut spawner.rng, SPAWN_ZONE_COLS),
        Some(a) if spawner.rng.random_range(0.0..1.0) >= JUMP_PROB => {
            step_anchor(a, &mut spawner.rng, ANCHOR_WALK_MIN, ANCHOR_WALK_MAX, SPAWN_ZONE_COLS)
        }
        Some(_) => random_anchor(&mut spawner.rng, SPAWN_ZONE_COLS),
    };
    spawner.anchor = Some(anchor);
    let cells = scatter(anchor, size, &mut spawner.rng, HERD_SPREAD);

    herds.cohorts.insert(code, Cohort { slot, spawned: size as u32, ..default() });
    herds.order.push(code);
    flows.births += size as f32;

    let Herds { cohorts, order } = &mut *herds;
    while order.len() > MAX_COHORTS {
        let victim = order
            .iter()
            .copied()
            .find(|c| cohorts.get(c).is_none_or(|co| co.alive == 0));
        let Some(code) = victim else { break };
        order.retain(|&c| c != code);
        cohorts.remove(&code);
    }

    spawn_cohort(&mut commands, slot, code, &cells, &mut spawner.rng);

    spawner.next_pack = (slot + 1) % super::PACK_COUNT as u8;
    spawner.cooldown = WAVE_INTERVAL;
}

/// Auto-spawn waves only on the procedural world; test maps spawn by hand via the UI.
/// `Option` so probe apps without `WorldgenPlugin` (no `WorldSource`) keep auto-spawning.
pub(super) fn auto_spawn_enabled(source: Option<Res<WorldSource>>) -> bool {
    !matches!(source.as_deref(), Some(WorldSource::TestMap(_)))
}

/// Scatter `count` elk near the left edge, vertically centred — works on any grid size.
fn left_zone_cells(grid: &Grid, count: u32, rng: &mut impl Rng) -> Vec<usize> {
    let (w, h) = (grid.width(), grid.height());
    let anchor_col = (w as f32 * 0.04).round().max(1.0);
    let anchor_row = h as f32 / 2.0;
    let spread = (h as f32 / 5.0).max(2.0);
    (0..count)
        .map(|_| {
            let col = (anchor_col + rng.random_range(-1.5..1.5)).round().clamp(0.0, (w - 1) as f32) as usize;
            let row = (anchor_row + rng.random_range(-spread..spread)).round().clamp(0.0, (h - 1) as f32) as usize;
            row * w + col
        })
        .collect()
}

/// Spawn one cohort of `ManualSpawn.count` elk when the UI button has fired.
pub(super) fn manual_spawn(
    mut commands: Commands,
    mut req: ResMut<ManualSpawn>,
    mut spawner: ResMut<Spawner>,
    mut herds: ResMut<Herds>,
    mut flows: ResMut<EnergyFlows>,
    grid: Res<Grid>,
) {
    if !req.fire {
        return;
    }
    req.fire = false;
    let count = req.count.clamp(1, 100);

    let spawner = &mut *spawner;
    let slot = spawner.next_pack;
    let code = spawner.rng.random_range(0..0x0100_0000u32);
    let cells = left_zone_cells(&grid, count, &mut spawner.rng);

    herds.cohorts.insert(code, Cohort { slot, spawned: count, ..default() });
    herds.order.push(code);
    flows.births += count as f32;

    spawn_cohort(&mut commands, slot, code, &cells, &mut spawner.rng);
    spawner.next_pack = (slot + 1) % super::PACK_COUNT as u8;
}

/// Despawns elk and resets lifecycle resources. `ElkParams` deliberately left untouched.
pub(super) fn teardown(
    mut commands: Commands,
    elk: Query<Entity, With<Elk>>,
    mut spawner: ResMut<Spawner>,
    mut herds: ResMut<Herds>,
) {
    for e in &elk {
        commands.entity(e).despawn();
    }
    *spawner = Spawner::default();
    *herds = Herds::default();
}

pub(super) fn cull(
    mut commands: Commands,
    grid: Res<Grid>,
    mut herds: ResMut<Herds>,
    mut elk: Query<(Entity, &mut Elk)>,
    mut flows: ResMut<EnergyFlows>,
    mut event_log: ResMut<EventLog>,
    spawner: Res<Spawner>,
) {
    // Runtime grid width, not compile-time GRID_WIDTH — works on any map size.
    let width = grid.width();
    let edge_col = width.saturating_sub(2);
    for (entity, mut elk) in &mut elk {
        if elk.cell % width >= edge_col {
            elk.at_edge += 1;
            if elk.at_edge >= EDGE_TICKS {
                if let Some(c) = herds.cohorts.get_mut(&elk.code) {
                    c.departures += 1;
                }
                flows.departures_energy += elk.energy;
                event_log.push(Event {
                    tick: spawner.elapsed as u64,
                    cell: elk.cell,
                    kind: EventKind::Departed,
                    energy: elk.energy,
                    chosen_step: None,
                });
                commands.entity(entity).despawn();
            }
        } else {
            elk.at_edge = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::SmallRng;
    use rand::SeedableRng;

    fn rng() -> SmallRng {
        SmallRng::seed_from_u64(0xE1C)
    }

    #[test]
    fn herd_size_stays_in_jitter_band_and_positive() {
        let mut r = rng();
        for base in [1usize, 5, 20, 50] {
            for _ in 0..200 {
                let s = herd_size(base, &mut r, SIZE_JITTER);
                assert!(s >= 1, "size {s} fell below 1 for base {base}");
                let hi = (base as f32 * (1.0 + SIZE_JITTER)).round() as usize;
                assert!(s <= hi, "size {s} exceeded jitter ceiling {hi} for base {base}");
            }
        }
    }

    #[test]
    fn herd_size_actually_varies() {
        let mut r = rng();
        let sizes: std::collections::HashSet<usize> =
            (0..50).map(|_| herd_size(20, &mut r, SIZE_JITTER)).collect();
        assert!(sizes.len() > 1, "herd sizes should vary, got {sizes:?}");
    }

    #[test]
    fn random_anchor_lands_in_zone() {
        let mut r = rng();
        for _ in 0..500 {
            let a = random_anchor(&mut r, SPAWN_ZONE_COLS);
            assert!((0.0..SPAWN_ZONE_COLS).contains(&a.x), "col {} out of zone", a.x);
            assert!((0.0..GRID_HEIGHT as f32).contains(&a.y), "row {} out of grid", a.y);
        }
    }

    #[test]
    fn step_anchor_stays_in_grid() {
        let mut r = rng();
            let mut a = Vec2::new(0.0, 0.0);
        for _ in 0..500 {
            a = step_anchor(a, &mut r, ANCHOR_WALK_MIN, ANCHOR_WALK_MAX, SPAWN_ZONE_COLS);
            assert!((0.0..SPAWN_ZONE_COLS).contains(&a.x), "col {} escaped zone", a.x);
            assert!((0.0..=(GRID_HEIGHT - 1) as f32).contains(&a.y), "row {} escaped grid", a.y);
        }
    }

    #[test]
    fn step_anchor_always_displaces_vertically() {
        let mut r = rng();
        // Mid-row start avoids edge reflection so minimum step is always observable.
        let start = Vec2::new(2.0, (GRID_HEIGHT / 2) as f32);
        for _ in 0..500 {
            let next = step_anchor(start, &mut r, ANCHOR_WALK_MIN, ANCHOR_WALK_MAX, SPAWN_ZONE_COLS);
            let dy = (next.y - start.y).abs();
            assert!(dy >= ANCHOR_WALK_MIN, "vertical step {dy} below the minimum");
        }
    }

    #[test]
    fn seed_anchor_is_vertically_centered() {
        let mut r = rng();
        let a = seed_anchor(&mut r, SPAWN_ZONE_COLS);
        assert_eq!(a.y, (GRID_HEIGHT / 2) as f32);
        assert!((0.0..SPAWN_ZONE_COLS).contains(&a.x));
    }

    #[test]
    fn scatter_yields_size_cells_inside_grid() {
        let mut r = rng();
        let anchor = Vec2::new(2.0, 50.0);
        let cells = scatter(anchor, 30, &mut r, HERD_SPREAD);
        assert_eq!(cells.len(), 30);
        for c in cells {
            assert!(c < GRID_WIDTH * GRID_HEIGHT, "cell {c} out of grid");
        }
    }

    #[test]
    fn scatter_clamps_at_grid_corner() {
        let mut r = rng();
        let cells = scatter(Vec2::new(0.0, 0.0), 50, &mut r, HERD_SPREAD);
        for c in cells {
            let (col, row) = (c % GRID_WIDTH, c / GRID_WIDTH);
            assert!(col < GRID_WIDTH && row < GRID_HEIGHT, "cell {c} escaped corner clamp");
        }
    }
}
