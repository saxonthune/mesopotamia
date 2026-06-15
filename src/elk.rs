use bevy::prelude::*;
use rand::Rng;
use std::collections::HashMap;

use crate::grid::{GRID_HEIGHT, GRID_WIDTH, Grid};

const TILE_SIZE: f32 = 16.0;

pub struct ElkPlugin;

pub const PACK_COUNT: usize = 8;

// Population lifecycle tuning.
const TARGET_POPULATION: usize = 200; // fixed headcount — does NOT scale with grid area,
// so a bigger world means the same herds spread thinner rather than 4× more elk
const WAVE_INTERVAL: u32 = 180; // ticks between successive spawn waves
const PACK_BASE: usize = 20; // elk per wave at the start
const TARGET_GROWTH_PERIOD: u32 = 1200; // +1 to target population per this many ticks
const PACK_GROWTH_PERIOD: u32 = 2400; // +1 to wave size per this many ticks
const EDGE_COL: usize = GRID_WIDTH - 2; // the two farthest columns count as "at the edge"
const EDGE_TICKS: u32 = 3; // consecutive ticks in that band before the elk leaves the map

impl Plugin for ElkPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Packs::new())
            .init_resource::<Spawner>()
            .init_resource::<ElkParams>()
            .init_resource::<Herds>()
            .add_systems(Update, (sync_elk_transform, sync_elk_color, tally_herds))
            .add_systems(
                FixedUpdate,
                (
                    herd_move,
                    graze,
                    digest,
                    metabolize,
                    migrate_pressure,
                    spawn_waves,
                    cull,
                ),
            );
    }
}

#[derive(Component)]
pub struct Elk {
    pub cell: usize,
    /// The cell the elk occupied before its last move. The simulation only ever
    /// reads `cell`; this exists purely so the renderer can slide the sprite from
    /// the old cell to the new one across a tick instead of snapping.
    pub prev_cell: usize,
    /// Recycled pack *slot* (0..PACK_COUNT); indexes `Packs` and `HerdStats`.
    pub slot: u8,
    /// Stable cohort identity, shown as a 6-digit hex code. Unlike `slot`, this
    /// is unique per spawned cohort and never recycled.
    pub code: u32,
    /// Drive signal in [0, 1]; drains each tick, replenished by grazing. The
    /// future decision AI reads this to decide what the elk is trying to do.
    pub energy: f32,
    /// Ticks remaining until each pending defecation. A meal eaten now lands
    /// here and is deposited as poop wherever the elk is when its timer expires.
    pub digesting: Vec<u32>,
    /// True on a tick the elk fed — the beacon other elk follow (social foraging).
    pub grazing: bool,
    /// Consecutive ticks spent at the far edge; once it crosses a threshold the
    /// elk has "left the map" and despawns.
    pub at_edge: u32,
}

/// Drives the herd lifecycle: spaces spawns into waves and keeps the headcount
/// near a target that itself grows over time.
#[derive(Resource, Default)]
pub struct Spawner {
    /// Ticks until the next wave is allowed.
    cooldown: u32,
    /// Rolling pack id handed to the next cohort.
    next_pack: u8,
    /// Total ticks elapsed; drives the growth of the spawn rules.
    elapsed: u32,
}

/// A spawned cohort's lifetime record, keyed by its hex `code`. Retained after
/// the cohort dies out so a panel can stay parked on a dead/migrated herd.
#[derive(Default, Clone)]
pub struct Cohort {
    pub slot: u8,
    pub alive: u32,
    pub energy_sum: f32, // average energy = energy_sum / alive
    pub peak: u32,       // largest the cohort ever was
    pub deaths: u32,     // starvations
    pub departures: u32, // migrations off the far edge
}

/// Registry of every (recent) cohort, keyed by hex code. `order` is spawn order,
/// used for stable dropdown listing and for pruning the oldest dead cohorts.
#[derive(Resource, Default)]
pub struct Herds {
    pub cohorts: HashMap<u32, Cohort>,
    pub order: Vec<u32>,
}

const MAX_COHORTS: usize = 64; // retained cohort records before pruning the dead

/// Recompute live counts and energy totals for every cohort each frame.
fn tally_herds(mut herds: ResMut<Herds>, elk: Query<&Elk>) {
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

/// Per-pack state. Each pack migrates toward the far side under its own,
/// independently growing pressure, so packs depart on staggered clocks.
#[derive(Resource)]
pub struct Packs {
    /// Current migration pressure per pack (pushes +x toward the far edge).
    pub migration: Vec<f32>,
    /// Per-pack growth added to `migration` each tick.
    pub growth: Vec<f32>,
}

impl Packs {
    fn new() -> Self {
        let migration = vec![0.0; PACK_COUNT];
        // A unitless per-pack multiplier on the global migration growth rate;
        // reseeded per cohort at spawn. Staggers when each pack departs.
        let growth = (0..PACK_COUNT)
            .map(|i| 0.5 + i as f32 / PACK_COUNT as f32)
            .collect();
        Self { migration, growth }
    }
}

/// An elk's tint. Hues sweep the warm/purple arc — purple → magenta → red →
/// orange-brown — deliberately skipping the green/blue half of the wheel so a
/// pack never blends into the grass, water, or dirt behind it. Adjacent slots
/// alternate lightness so neighbouring hues still separate, and a grazing elk
/// dims just enough to read as feeding without losing its pack hue.
fn elk_color(slot: usize, grazing: bool) -> Color {
    const ARC: f32 = 120.0; // 285° (purple) through 405°≡45° (orange-brown)
    let hue = (285.0 + slot as f32 / (PACK_COUNT - 1) as f32 * ARC) % 360.0;
    let mut light = 0.5 + 0.1 * (slot % 2) as f32;
    if grazing {
        light *= 0.78; // a gentle dim, not a blackout
    }
    Color::hsl(hue, 0.7, light)
}

/// Spawn one cohort of `size` elk — pack `code`, occupying `slot`'s vertical
/// band on the left edge.
fn spawn_pack(commands: &mut Commands, rng: &mut impl Rng, slot: u8, code: u32, size: usize) {
    let color = elk_color(slot as usize, false);
    let band = GRID_HEIGHT / PACK_COUNT; // rows per slot
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

/// Spawns cohorts in spaced waves whenever the population sits below a target.
/// Both the target and the wave size creep upward with elapsed time.
fn spawn_waves(
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

    spawner.next_pack = (slot + 1) % PACK_COUNT as u8;
    spawner.cooldown = WAVE_INTERVAL;
}

/// Despawns any elk that has lingered at the far edge long enough to count as
/// having left the map. Elk despawn individually, so a partial pack empties out.
fn cull(
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

/// World-space centre of a cell. Mirrors `render::cell_world_pos` but reads the
/// grid dimensions from constants, so the elk renderer needs no `Grid` handle.
fn cell_pos(cell: usize) -> Vec2 {
    let col = (cell % GRID_WIDTH) as f32;
    let row = (cell / GRID_WIDTH) as f32;
    Vec2::new(
        (col - GRID_WIDTH as f32 / 2.0 + 0.5) * TILE_SIZE,
        (row - GRID_HEIGHT as f32 / 2.0 + 0.5) * TILE_SIZE,
    )
}

/// Slide each sprite from its previous cell to its current one across the tick.
/// `overstep_fraction` is how far we are through the current 10 Hz step [0, 1),
/// so the sprite glides at render rate while the simulation still steps discretely.
fn sync_elk_transform(time: Res<Time<Fixed>>, mut elk: Query<(&Elk, &mut Transform)>) {
    let t = time.overstep_fraction();
    for (elk, mut transform) in &mut elk {
        let pos = cell_pos(elk.prev_cell).lerp(cell_pos(elk.cell), t);
        transform.translation.x = pos.x;
        transform.translation.y = pos.y;
    }
}

/// Recolour each elk from the `grazing` flag the simulation sets, so a feeding
/// elk visibly darkens. Render-only: reads sim state, writes nothing back.
fn sync_elk_color(mut elk: Query<(&Elk, &mut Sprite)>) {
    for (elk, mut sprite) in &mut elk {
        sprite.color = elk_color(elk.slot as usize, elk.grazing);
    }
}

const OTHER_PACK_SEP: f32 = 0.5; // mild push from foreign packs
const POOP_PER_GRAZE: f32 = 0.3;
const DIGEST_TICKS: u32 = 20;

/// Every tunable that shapes how elk move and feed. Each movement drive is a
/// pure weight on a *normalized* direction, so the weights are real ratios that
/// hold their meaning across grid sizes and densities. Radii are absolute
/// perception lengths (an elk's senses do not scale with the map).
#[derive(Resource)]
pub struct ElkParams {
    pub separation: f32,
    pub cohesion: f32,
    pub grass: f32,     // attraction up the grass gradient
    pub social: f32,    // attraction toward elk seen grazing (local enhancement)
    pub migration: f32, // weight of the far-edge fallback pull
    pub sep_radius: f32,
    pub coh_radius: f32,
    pub grass_radius: f32,
    pub social_radius: f32, // how far a grazing elk is noticed — longer than grass
    pub temperature: f32,   // step randomness; higher = more diffuse
    pub bite: f32,        // grass eaten per graze
    pub energy_per_bite: f32,
    pub energy_drain: f32, // energy lost per tick; reaching 0 starves the elk
    pub mig_growth: f32,   // per-tick growth of migration pressure (0 → 1 ramp)
    pub water_cost: f32,     // step penalty for entering water — fording is costly
    pub ford_discount: f32,  // fraction of water_cost paid on a ford (0 → free, 1 → full cost)
    pub swim_drain: f32,     // energy drained when entering deep non-ford water
    pub browse_bite: f32,    // browse stripped per graze — a big bite
    pub browse_energy: f32,  // energy from a browse bite — concentrated forage
}

impl Default for ElkParams {
    fn default() -> Self {
        Self {
            separation: 1.0,
            cohesion: 0.6,
            grass: 1.4,
            social: 0.8,
            migration: 0.35,
            sep_radius: 3.0,
            coh_radius: 9.0,
            grass_radius: 5.0,
            social_radius: 16.0,
            temperature: 0.6,
            bite: 0.5,
            // Sits a few × above the break-even grazing fraction (drain / this),
            // so a well-fed herd thrives but an overgrazed one starves — instead
            // of the old 0.2, which was ~50× break-even and pinned energy at the cap.
            energy_per_bite: 0.012,
            energy_drain: 0.004,
            mig_growth: 0.0015,
            water_cost: 2.0,
            ford_discount: 0.1,
            swim_drain: 0.01,
            browse_bite: 0.34,
            browse_energy: 0.05,
        }
    }
}

/// Step cost for entering a water cell. On a ford the cost is reduced by
/// `ford_discount`; off a ford it is the raw water-level × cost (today's behaviour).
pub fn step_water_penalty(water: f32, is_ford: bool, water_cost: f32, ford_discount: f32) -> f32 {
    let base = water * water_cost;
    if is_ford { base * ford_discount } else { base }
}

/// Unit vector in `v`'s direction, or zero if `v` is ~zero.
fn norm(v: Vec2) -> Vec2 {
    let len = v.length();
    if len > 1e-6 { v / len } else { Vec2::ZERO }
}

/// Two-phase boids-on-a-lattice. Phase 1 snapshots every elk's position and
/// pack; phase 2 builds a desire vector from four *normalized* drives —
/// separation, cohesion, grass-gradient, and the migration fallback — then
/// picks a step by softmax weighted-random.
fn herd_move(
    grid: Res<Grid>,
    packs: Res<Packs>,
    params: Res<ElkParams>,
    mut elk_q: Query<&mut Elk>,
) {
    // Phase 1: snapshot (col, row, slot, grazing) for every elk, in query order.
    let snapshot: Vec<(f32, f32, u8, bool)> = elk_q
        .iter()
        .map(|elk| {
            let (col, row) = grid.col_row(elk.cell);
            (col as f32, row as f32, elk.slot, elk.grazing)
        })
        .collect();

    // Phase 2: decide and write, reading neighbours only from the snapshot.
    let mut rng = rand::rng();
    let steps: [(isize, isize); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];
    let gr = params.grass_radius.ceil() as isize;

    for (i, mut elk) in elk_q.iter_mut().enumerate() {
        // Where the elk starts this tick becomes the render interpolation's
        // origin; if it doesn't move below, prev == cell and the sprite holds still.
        elk.prev_cell = elk.cell;
        let (cx, cy, slot, _) = snapshot[i];
        let pos = Vec2::new(cx, cy);

        // Neighbour drives: separation (repel), cohesion (pull to same-slot
        // centroid), and social foraging (pull toward any grazing elk seen far).
        let mut sep = Vec2::ZERO;
        let mut coh_sum = Vec2::ZERO;
        let mut coh_n = 0.0_f32;
        let mut social = Vec2::ZERO;
        for (j, &(ox, oy, oslot, ograzing)) in snapshot.iter().enumerate() {
            if j == i {
                continue;
            }
            let off = Vec2::new(cx - ox, cy - oy);
            let d2 = off.length_squared();
            if d2 == 0.0 {
                continue;
            }
            let dist = d2.sqrt();
            if dist <= params.sep_radius {
                let factor = if oslot == slot { 1.0 } else { OTHER_PACK_SEP };
                sep += off / d2 * factor; // push away, harder the closer
            }
            if oslot == slot && dist <= params.coh_radius {
                coh_sum += Vec2::new(ox, oy);
                coh_n += 1.0;
            }
            // Local enhancement: grazing elk (any pack) draw foragers from afar,
            // nearer ones more — `-off` points from self toward the grazer.
            if ograzing && dist <= params.social_radius {
                social += (-off) / dist / dist;
            }
        }
        let coh = if coh_n > 0.0 { coh_sum / coh_n - pos } else { Vec2::ZERO };

        // Field drive: steer up the grass gradient, near and rich grass weighing
        // most. As a herd eats a hole, this points outward to fresh forage.
        let mut grass_dir = Vec2::ZERO;
        for dy in -gr..=gr {
            for dx in -gr..=gr {
                if dx == 0 && dy == 0 {
                    continue;
                }
                if let Some(n) = grid.step(elk.cell, dx, dy) {
                    let off = Vec2::new(dx as f32, dy as f32);
                    let dist = off.length();
                    if dist > params.grass_radius {
                        continue;
                    }
                    grass_dir += off / dist * (grid.forage(n) / dist);
                }
            }
        }

        // Combine normalized drives. Hunger sharpens the pull toward food —
        // both grass directly and other elk already feeding — so a fed herd
        // drifts while a starving one bolts toward the nearest feast.
        let hunger = 1.0 - elk.energy;
        let appetite = 0.25 + 0.75 * hunger;
        let desire = norm(sep) * params.separation
            + norm(coh) * params.cohesion
            + norm(grass_dir) * (params.grass * appetite)
            + norm(social) * (params.social * appetite)
            + Vec2::X * (params.migration * packs.migration[slot as usize]);

        // Score each valid step, then softmax for a weighted-random pick.
        let mut scores = [f32::NEG_INFINITY; 4];
        let mut cells: [Option<usize>; 4] = [None; 4];
        for (k, &(dx, dy)) in steps.iter().enumerate() {
            if let Some(next) = grid.step(elk.cell, dx, dy) {
                // Fording is costly — deep water repels, a ford less so. This
                // is what turns a crossing into a decision: a herd only steps into
                // water when the forage drive beyond outweighs the penalty.
                scores[k] = desire.dot(Vec2::new(dx as f32, dy as f32))
                    - step_water_penalty(
                        grid.water(next),
                        grid.is_ford(next),
                        params.water_cost,
                        params.ford_discount,
                    );
                cells[k] = Some(next);
            }
        }
        let max = scores.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        if !max.is_finite() {
            continue; // hemmed in (corner with no valid step)
        }
        let mut weights = [0.0_f32; 4];
        for k in 0..4 {
            if scores[k].is_finite() {
                weights[k] = ((scores[k] - max) / params.temperature).exp();
            }
        }

        // The max-scored step always has weight 1, so total >= 1 (no div-by-0).
        let total: f32 = weights.iter().sum();
        let mut pick = rng.random_range(0.0..total);
        for k in 0..4 {
            let Some(next) = cells[k] else { continue };
            if pick < weights[k] {
                elk.cell = next;
                break;
            }
            pick -= weights[k];
        }

        // Swim energy cost: crossing deep non-ford water is a lasting risk beyond
        // the step-score penalty — the elk arrives tired.
        let water = grid.water(elk.cell);
        if water > 0.0 && !grid.is_ford(elk.cell) {
            elk.energy = (elk.energy - params.swim_drain * water).max(0.0);
        }
    }
}

fn migrate_pressure(params: Res<ElkParams>, mut packs: ResMut<Packs>) {
    for i in 0..packs.migration.len() {
        let g = packs.growth[i] * params.mig_growth;
        packs.migration[i] = (packs.migration[i] + g).min(1.0);
    }
}

fn graze(mut grid: ResMut<Grid>, params: Res<ElkParams>, mut elk: Query<&mut Elk>) {
    for mut elk in &mut elk {
        if grid.browse(elk.cell) > 0.05 {
            // Browse first: a big, concentrated bite that strips the shrub and
            // pays more energy than grass — the reward for crossing dry ground.
            grid.eat_browse(elk.cell, params.browse_bite);
            elk.energy = (elk.energy + params.browse_energy).min(1.0);
            elk.digesting.push(DIGEST_TICKS);
            elk.grazing = true;
        } else if grid.grass(elk.cell) > 0.0 {
            grid.grow_grass(elk.cell, -params.bite);
            elk.energy = (elk.energy + params.energy_per_bite).min(1.0);
            elk.digesting.push(DIGEST_TICKS);
            elk.grazing = true; // raise the beacon other elk forage toward
        } else {
            elk.grazing = false;
        }
    }
}

/// Counts down each pending meal; when one expires, drop poop at the elk's
/// *current* cell — so nutrients move with the body, not back to the eat-site.
fn digest(mut grid: ResMut<Grid>, mut elk: Query<&mut Elk>) {
    for mut elk in &mut elk {
        let cell = elk.cell;
        elk.digesting.retain_mut(|t| {
            if *t == 0 {
                grid.add_poop(cell, POOP_PER_GRAZE);
                false
            } else {
                *t -= 1;
                true
            }
        });
    }
}

/// Every tick the elk spends energy just being alive; an elk that runs out
/// starves and despawns — the consequence that drives the herd to keep feeding.
fn metabolize(
    mut commands: Commands,
    params: Res<ElkParams>,
    mut herds: ResMut<Herds>,
    mut elk: Query<(Entity, &mut Elk)>,
) {
    for (entity, mut elk) in &mut elk {
        elk.energy -= params.energy_drain;
        if elk.energy <= 0.0 {
            if let Some(c) = herds.cohorts.get_mut(&elk.code) {
                c.deaths += 1;
            }
            commands.entity(entity).despawn();
        }
    }
}

/// The verifiable behavioural contract for river crossings (world-gen C): a herd's
/// net incentive to ford to the far bank rather than stay on its current side.
/// Forage terms are in [0, 1] — `here` under the herd, `ahead` the next forage along
/// its migration axis (+x), `across` the forage on the far bank (+y over the water);
/// `cross_cost` is the effective ford price. Positive ⇒ the far bank beats the best
/// dry option net of the crossing, so the herd wants to cross.
///
/// The model: a herd takes the richest *reachable* forage and only pays to cross when
/// nothing cheaper rivals the far bank — so a lead herd with fresh grass still ahead
/// stays, while a trailing herd facing a grazed-out corridor crosses. This is the
/// extracted, testable kernel of the crossing decision the full softmax move expresses;
/// the metamorphic tests below pin its behaviour, and the fording work in task
/// `water-as-barrier-fords` consumes it directly.
#[allow(dead_code)] // a verified behavioural spec, wired into movement in task B
pub fn cross_desire(here: f32, ahead: f32, across: f32, cross_cost: f32) -> f32 {
    across - here.max(ahead) - cross_cost
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── step_water_penalty ────────────────────────────────────────────────────

    // On a ford the penalty collapses to nearly zero regardless of water depth.
    #[test]
    fn ford_penalty_is_near_zero() {
        let cost = step_water_penalty(1.0, true, 2.0, 0.1);
        assert!(cost < 0.25, "ford should be nearly free, got {cost}");
    }

    // Off a ford the penalty is identical to today's plain water × cost.
    #[test]
    fn non_ford_penalty_equals_water_times_cost() {
        let cost = step_water_penalty(0.7, false, 2.0, 0.1);
        assert!((cost - 0.7 * 2.0).abs() < 1e-6);
    }

    // Metamorphic: penalty off a ford is monotone increasing in water level.
    #[test]
    fn penalty_monotone_in_water() {
        let low = step_water_penalty(0.2, false, 2.0, 0.1);
        let high = step_water_penalty(0.8, false, 2.0, 0.1);
        assert!(high > low);
    }

    // Deep non-ford water costs strictly more than a shallow tributary.
    #[test]
    fn deep_non_ford_beats_shallow_tributary() {
        let tributary = step_water_penalty(0.1, false, 2.0, 0.1); // shallow
        let deep = step_water_penalty(1.0, false, 2.0, 0.1);      // main channel
        assert!(deep > tributary);
    }

    // ── cross_desire ─────────────────────────────────────────────────────────

    // Example scenario — a lead herd: fresh grass both ahead (+x) and across (+y).
    // Advancing along +x is as good as crossing and costs nothing, so it stays.
    #[test]
    fn lead_herd_with_forage_ahead_does_not_cross() {
        assert!(cross_desire(0.5, 0.9, 0.9, 0.2) <= 0.0);
    }

    // Example scenario — a trailing herd: the lead ate the +x corridor, so `here`
    // and `ahead` are bare; only the far bank is green. It crosses.
    #[test]
    fn trailing_herd_crosses_for_the_far_bank() {
        assert!(cross_desire(0.1, 0.1, 0.9, 0.2) > 0.0);
    }

    // Metamorphic relation — richer forage AHEAD can only lower the urge to cross:
    // a better dry option competes with the far bank. (Monotone ↓ in `ahead`.)
    #[test]
    fn more_forage_ahead_never_raises_cross_desire() {
        assert!(cross_desire(0.1, 0.8, 0.9, 0.2) <= cross_desire(0.1, 0.2, 0.9, 0.2));
    }

    // Metamorphic relation — richer forage ACROSS can only raise it. (Monotone ↑.)
    #[test]
    fn more_forage_across_never_lowers_cross_desire() {
        assert!(cross_desire(0.1, 0.1, 0.9, 0.2) >= cross_desire(0.1, 0.1, 0.4, 0.2));
    }

    // Metamorphic relation — a costlier ford can only lower it. (Monotone ↓ in cost.)
    #[test]
    fn costlier_crossing_never_raises_cross_desire() {
        assert!(cross_desire(0.1, 0.1, 0.9, 0.5) <= cross_desire(0.1, 0.1, 0.9, 0.1));
    }
}
