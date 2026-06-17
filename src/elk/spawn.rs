use bevy::prelude::*;
use rand::Rng;

use crate::grid::{GRID_HEIGHT, GRID_WIDTH};

use super::components::{Cohort, DriveSample, DriveSamples, Elk, Herds, Packs, Spawner, MAX_COHORTS};
use super::color::elk_color;

const TILE_SIZE: f32 = 16.0;

// Population lifecycle tuning.
pub const TARGET_POPULATION: usize = 200; // fixed headcount — does NOT scale with grid area,
// so a bigger world means the same herds spread thinner rather than 4× more elk
const WAVE_INTERVAL: u32 = 180; // ticks between successive spawn waves
const PACK_BASE: usize = 20; // elk per wave at the start
const TARGET_GROWTH_PERIOD: u32 = 1200; // +1 to target population per this many ticks
const PACK_GROWTH_PERIOD: u32 = 2400; // +1 to wave size per this many ticks
pub const EDGE_COL: usize = GRID_WIDTH - 2; // the two farthest columns count as "at the edge"
const EDGE_TICKS: u32 = 3; // consecutive ticks in that band before the elk leaves the map

/// Spawn one cohort of `size` elk — pack `code`, occupying `slot`'s vertical
/// band on the left edge.
fn spawn_pack(commands: &mut Commands, rng: &mut impl Rng, slot: u8, code: u32, size: usize) {
    let color = elk_color(slot as usize, false);
    let band = GRID_HEIGHT / super::PACK_COUNT; // rows per slot
    let lo = slot as usize * band;
    for _ in 0..size {
        let col = rng.random_range(0..6);
        let row = rng.random_range(lo..lo + band);
        let cell = row * GRID_WIDTH + col;
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
            },
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
) {
    spawner.elapsed += 1;
    if spawner.cooldown > 0 {
        spawner.cooldown -= 1;
        return;
    }

    // Spawn rules grow slowly over time, from a grid-independent base.
    let target = TARGET_POPULATION + (spawner.elapsed / TARGET_GROWTH_PERIOD) as usize;
    let size = PACK_BASE + (spawner.elapsed / PACK_GROWTH_PERIOD) as usize;

    if elk.iter().count() >= target {
        return;
    }

    let mut rng = rand::rng();
    let slot = spawner.next_pack;
    let code = rng.random_range(0..0x0100_0000u32); // 6 hex digits

    // A reused slot starts a fresh cohort: reset its migration pressure and
    // reseed growth. The old cohort's record lives on in `herds`.
    packs.migration[slot as usize] = 0.0;
    packs.growth[slot as usize] = rng.random_range(0.5..1.5);

    herds.cohorts.insert(code, Cohort { slot, ..default() });
    herds.order.push(code);

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

    spawn_pack(&mut commands, &mut rng, slot, code, size);

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
) {
    for (entity, mut elk) in &mut elk {
        if elk.cell % GRID_WIDTH >= EDGE_COL {
            elk.at_edge += 1;
            if elk.at_edge >= EDGE_TICKS {
                if let Some(c) = herds.cohorts.get_mut(&elk.code) {
                    c.departures += 1;
                }
                commands.entity(entity).despawn();
            }
        } else {
            elk.at_edge = 0;
        }
    }
}
