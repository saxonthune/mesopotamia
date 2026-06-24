use bevy::prelude::*;
use rand::Rng;

use crate::events::{Event, EventKind, EventLog};
use crate::grid::{GRID_HEIGHT, GRID_WIDTH};

use super::components::{Cohort, DriveSample, DriveSamples, Elk, Herds, LastDecision, Packs, Spawner, MAX_COHORTS};
use super::ledger::EnergyFlows;
use super::movement::Decision;
use super::color::elk_color;

const TILE_SIZE: f32 = 16.0;

// Population lifecycle tuning.
pub const TARGET_POPULATION: usize = 200; // fixed headcount — does NOT scale with grid area,
// so a bigger world means the same herds spread thinner rather than 4× more elk
const MAX_POPULATION: usize = 240; // hard ceiling: spawning stops above this no matter how
// far the creeping target has climbed
const WAVE_INTERVAL: u32 = 280; // ticks between successive spawn waves
const PACK_BASE: usize = 20; // elk per wave at the start
const TARGET_GROWTH_PERIOD: u32 = 1200; // +1 to target population per this many ticks
const PACK_GROWTH_PERIOD: u32 = 2400; // +1 to wave size per this many ticks
pub const EDGE_COL: usize = GRID_WIDTH - 2; // the two farthest columns count as "at the edge"
const EDGE_TICKS: u32 = 3; // consecutive ticks in that band before the elk leaves the map

// Spawn-position policy. Herds spawn from a single anchor that random-walks
// inside a left-edge zone, so successive packs trail one another along the path
// of grass the last pack grazed — with an occasional jump to a fresh spot.
const SPAWN_ZONE_COLS: f32 = 6.0; // anchor stays within the left N columns
const ANCHOR_WALK_MIN: f32 = 18.0; // each wave's anchor moves at least this far up/down...
const ANCHOR_WALK_MAX: f32 = 40.0; // ...and at most this far, so consecutive herds don't pile up
const HERD_SPREAD: f32 = 3.0; // blob half-width scattered around the anchor
const SIZE_JITTER: f32 = 0.5; // herd size varies by ±this fraction of the base
const JUMP_PROB: f32 = 0.2; // chance each wave to reseed the anchor at random

/// Vary a wave's headcount: `base` scaled by a uniform factor in
/// `[1 - jitter, 1 + jitter]`, never below 1.
fn herd_size(base: usize, rng: &mut impl Rng, jitter: f32) -> usize {
    let factor = rng.random_range(1.0 - jitter..1.0 + jitter);
    ((base as f32 * factor).round() as usize).max(1)
}

/// The first wave's anchor: centered vertically, random column within the zone.
fn seed_anchor(rng: &mut impl Rng, zone_cols: f32) -> Vec2 {
    Vec2::new(rng.random_range(0.0..zone_cols), (GRID_HEIGHT / 2) as f32)
}

/// A fresh anchor anywhere in the left spawn zone (the occasional reseed jump).
fn random_anchor(rng: &mut impl Rng, zone_cols: f32) -> Vec2 {
    Vec2::new(
        rng.random_range(0.0..zone_cols),
        rng.random_range(0.0..GRID_HEIGHT as f32),
    )
}

/// Random-walk the anchor vertically by a *guaranteed* step in
/// `[min_walk, max_walk]` (up or down), reflecting off the top/bottom edges so
/// the displacement survives — consecutive herds never land on one another. The
/// column is re-jittered freely within the narrow zone.
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

/// Scatter `size` grid cells in a square blob of half-width `spread` around the
/// anchor, each clamped onto the grid.
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

/// Spawn one cohort of elk — pack `code`, slot `slot` (color/identity) — one
/// entity per cell in `cells`.
fn spawn_cohort(commands: &mut Commands, slot: u8, code: u32, cells: &[usize]) {
    let color = elk_color(slot as usize, false);
    for &cell in cells {
        commands.spawn((
            Sprite::from_color(color, Vec2::splat(TILE_SIZE * 0.7)),
            Transform::from_xyz(0.0, 0.0, 1.0),
            Elk {
                cell,
                prev_cell: cell,
                slot,
                code,
                energy: 1.0,
                digesting: Vec::new(),
                grazing: false,
                at_edge: 0,
                intake_rate: 0.0,
                traveling: false,
            },
            LastDecision(Decision::default()),
        ));
    }
}

/// Recompute live counts and energy totals for every cohort each frame.
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

/// Spawns cohorts in spaced waves whenever the population sits below a target.
/// Both the target and the wave size creep upward with elapsed time.
pub(super) fn spawn_waves(
    mut commands: Commands,
    mut spawner: ResMut<Spawner>,
    mut packs: ResMut<Packs>,
    mut herds: ResMut<Herds>,
    elk: Query<(), With<Elk>>,
    mut flows: ResMut<EnergyFlows>,
) {
    spawner.elapsed += 1;
    if spawner.cooldown > 0 {
        spawner.cooldown -= 1;
        return;
    }

    // The creeping target grows with time but is capped: spawning halts once the
    // population reaches the hard ceiling, however high the target has climbed.
    let target = (TARGET_POPULATION + (spawner.elapsed / TARGET_GROWTH_PERIOD) as usize)
        .min(MAX_POPULATION);
    let base = PACK_BASE + (spawner.elapsed / PACK_GROWTH_PERIOD) as usize;

    if elk.iter().count() >= target {
        return;
    }

    let mut rng = rand::rng();
    let slot = spawner.next_pack;
    let code = rng.random_range(0..0x0100_0000u32); // 6 hex digits
    let size = herd_size(base, &mut rng, SIZE_JITTER);

    // The first wave starts mid-height; later waves random-walk from the last
    // spot, with an occasional jump to a fresh spot so the trail doesn't ossify.
    let anchor = match spawner.anchor {
        None => seed_anchor(&mut rng, SPAWN_ZONE_COLS),
        Some(a) if rng.random_range(0.0..1.0) >= JUMP_PROB => {
            step_anchor(a, &mut rng, ANCHOR_WALK_MIN, ANCHOR_WALK_MAX, SPAWN_ZONE_COLS)
        }
        Some(_) => random_anchor(&mut rng, SPAWN_ZONE_COLS),
    };
    spawner.anchor = Some(anchor);
    let cells = scatter(anchor, size, &mut rng, HERD_SPREAD);

    // A reused slot starts a fresh cohort: reset its migration pressure and
    // reseed growth. The old cohort's record lives on in `herds`.
    packs.migration[slot as usize] = 0.0;
    packs.growth[slot as usize] = rng.random_range(0.5..1.5);

    herds.cohorts.insert(code, Cohort { slot, spawned: size as u32, ..default() });
    herds.order.push(code);
    flows.births += size as f32;

    // Prune the oldest *dead* cohorts once the registry grows past its cap;
    // never evict a living herd.
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

    spawn_cohort(&mut commands, slot, code, &cells);

    spawner.next_pack = (slot + 1) % super::PACK_COUNT as u8;
    spawner.cooldown = WAVE_INTERVAL;
}

/// On leaving `Running` (a regenerate), despawn every elk and reset the herd
/// lifecycle resources so the new world starts from an empty range. The tuned
/// `ElkParams` are deliberately left untouched.
pub(super) fn teardown(
    mut commands: Commands,
    elk: Query<Entity, With<Elk>>,
    mut spawner: ResMut<Spawner>,
    mut herds: ResMut<Herds>,
    mut packs: ResMut<Packs>,
    mut drive_samples: ResMut<DriveSamples>,
) {
    for e in &elk {
        commands.entity(e).despawn();
    }
    *spawner = Spawner::default();
    *herds = Herds::default();
    *packs = Packs::new();
    drive_samples.per_slot = vec![DriveSample::default(); super::PACK_COUNT];
}

/// Despawns any elk that has lingered at the far edge long enough to count as
/// having left the map. Elk despawn individually, so a partial pack empties out.
pub(super) fn cull(
    mut commands: Commands,
    mut herds: ResMut<Herds>,
    mut elk: Query<(Entity, &mut Elk)>,
    mut flows: ResMut<EnergyFlows>,
    mut event_log: ResMut<EventLog>,
    spawner: Res<Spawner>,
) {
    for (entity, mut elk) in &mut elk {
        if elk.cell % GRID_WIDTH >= EDGE_COL {
            elk.at_edge += 1;
            if elk.at_edge >= EDGE_TICKS {
                if let Some(c) = herds.cohorts.get_mut(&elk.code) {
                    c.departures += 1;
                }
                flows.departures_energy += elk.energy;
                // A departure is a scored win — the mirror of `Starved` in
                // `metabolize`. The score system folds both into the rating.
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
        // Start at a corner so the walk is pushed against the edges.
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
        // From a comfortable mid-row spot (no edge reflection), every step must
        // move at least the minimum so herds never spawn on top of each other.
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
        // Anchor in the top-left corner: negative offsets must clamp, not wrap.
        let cells = scatter(Vec2::new(0.0, 0.0), 50, &mut r, HERD_SPREAD);
        for c in cells {
            let (col, row) = (c % GRID_WIDTH, c / GRID_WIDTH);
            assert!(col < GRID_WIDTH && row < GRID_HEIGHT, "cell {c} escaped corner clamp");
        }
    }
}
