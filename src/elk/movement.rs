use bevy::prelude::*;
use rand::Rng;

use crate::grid::Grid;

use super::components::{DriveSample, DriveSamples, Elk, ElkParams, Packs};

const OTHER_PACK_SEP: f32 = 0.5; // mild push from foreign packs

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

/// Residual weight on the migration pull, in (0, 1]. Migration is a *fallback*:
/// it fills in only as the natural drives fall quiet. `quiet` is the crossover —
/// the `natural_strength` at which migration is at half weight. Pure and tiny so
/// the fade's shape is a one-line swap and the tests below still hold.
pub fn migration_residual(natural_strength: f32, quiet: f32) -> f32 {
    1.0 / (1.0 + natural_strength / quiet.max(1e-6))
}

/// The five weighted contribution vectors that sum into an elk's step desire,
/// kept separate so the migration-vs-natural split is legible and testable.
pub struct Drives {
    pub sep: Vec2,
    pub coh: Vec2,
    pub grass: Vec2,
    pub social: Vec2,
    pub migration: Vec2,
}

impl Drives {
    /// The combined desire vector — what `herd_move` steers by.
    pub fn total(&self) -> Vec2 {
        self.sep + self.coh + self.grass + self.social + self.migration
    }

    /// Fraction of total pull effort that is the migration ("magic") force, in
    /// [0, 1]: |migration| over the sum of all five component magnitudes. 0 when
    /// nothing pulls. Opposing drives that cancel still count toward the total.
    #[allow(dead_code)] // sampled per-tick by ui-declarative-panels-graphs and balancing-param-sweep
    pub fn migration_share(&self) -> f32 {
        let sum = self.sep.length()
            + self.coh.length()
            + self.grass.length()
            + self.social.length()
            + self.migration.length();
        if sum > 1e-6 {
            self.migration.length() / sum
        } else {
            0.0
        }
    }
}

/// Combine the raw per-drive direction accumulators into weighted contributions.
/// `sep`/`coh`/`grass_dir`/`social` are the un-normalized accumulators built in
/// `herd_move`; `pressure` is the pack's migration pressure; `energy` is the elk's
/// energy (hunger sharpens the food drives). Migration is residual — it scales by
/// how quiet the naturals are.
pub fn combine_drives(
    sep: Vec2,
    coh: Vec2,
    grass_dir: Vec2,
    social: Vec2,
    params: &ElkParams,
    pressure: f32,
    energy: f32,
) -> Drives {
    let hunger = 1.0 - energy;
    let appetite = 0.25 + 0.75 * hunger;
    let sep = norm(sep) * params.separation;
    let coh = norm(coh) * params.cohesion;
    let grass = norm(grass_dir) * (params.grass * appetite);
    let social = norm(social) * (params.social * appetite);
    let natural_strength = sep.length() + coh.length() + grass.length() + social.length();
    let migration = Vec2::X
        * (params.migration * pressure * migration_residual(natural_strength, params.quiet));
    Drives { sep, coh, grass, social, migration }
}

/// Two-phase boids-on-a-lattice. Phase 1 snapshots every elk's position and
/// pack; phase 2 builds a desire vector from four *normalized* drives —
/// separation, cohesion, grass-gradient, and the migration fallback — then
/// picks a step by softmax weighted-random.
pub(super) fn herd_move(
    grid: Res<Grid>,
    packs: Res<Packs>,
    params: Res<ElkParams>,
    mut elk_q: Query<&mut Elk>,
    mut samples: ResMut<DriveSamples>,
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
    let mut acc = vec![DriveSample::default(); super::PACK_COUNT];
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
        // Migration is a residual: it fills in only as the natural drives fall quiet.
        let drives = combine_drives(
            sep, coh, grass_dir, social, &params, packs.migration[slot as usize], elk.energy,
        );

        // Accumulate magnitudes for the per-slot sample (pure read, no behaviour change).
        let s = slot as usize;
        acc[s].sep += drives.sep.length();
        acc[s].coh += drives.coh.length();
        acc[s].grass += drives.grass.length();
        acc[s].social += drives.social.length();
        acc[s].migration += drives.migration.length();
        acc[s].count += 1;

        let desire = drives.total();

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

    // Write per-slot means into the shared resource. Empty slots get zeroed samples.
    for (slot_sample, a) in samples.per_slot.iter_mut().zip(acc.iter()) {
        if a.count > 0 {
            let n = a.count as f32;
            *slot_sample = DriveSample {
                sep: a.sep / n,
                coh: a.coh / n,
                grass: a.grass / n,
                social: a.social / n,
                migration: a.migration / n,
                count: a.count,
            };
        } else {
            *slot_sample = DriveSample::default();
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
    use super::super::components::DriveSample;

    // ── DriveSample::migration_share ─────────────────────────────────────────

    // 0 when all magnitudes are zero (idle elk).
    #[test]
    fn drive_sample_share_is_zero_when_idle() {
        let s = DriveSample::default();
        assert_eq!(s.migration_share(), 0.0);
    }

    // Matches the doc02.02 definition: migration / sum_of_five.
    #[test]
    fn drive_sample_share_matches_manual_calculation() {
        let s = DriveSample { sep: 1.0, coh: 1.0, grass: 1.0, social: 1.0, migration: 1.0, count: 1 };
        // All equal → share is exactly 0.2.
        assert!((s.migration_share() - 0.2).abs() < 1e-6);
    }

    // Pure migration drive (all others zero) → share is 1.
    #[test]
    fn drive_sample_share_is_one_when_only_migration() {
        let s = DriveSample { sep: 0.0, coh: 0.0, grass: 0.0, social: 0.0, migration: 2.0, count: 1 };
        assert!((s.migration_share() - 1.0).abs() < 1e-6);
    }

    // Raising migration while keeping others fixed raises the share.
    #[test]
    fn drive_sample_share_rises_with_migration() {
        let lo = DriveSample { sep: 1.0, coh: 1.0, grass: 1.0, social: 1.0, migration: 0.5, count: 1 };
        let hi = DriveSample { sep: 1.0, coh: 1.0, grass: 1.0, social: 1.0, migration: 2.0, count: 1 };
        assert!(hi.migration_share() > lo.migration_share());
    }

    fn default_params() -> ElkParams {
        ElkParams::default()
    }

    // ── migration_residual ────────────────────────────────────────────────────

    // At natural_strength = 0 the residual is exactly 1: migration is unattenuated.
    #[test]
    fn residual_is_one_when_drives_silent() {
        assert!((migration_residual(0.0, 1.0) - 1.0).abs() < 1e-6);
    }

    // At natural_strength == quiet the residual is 0.5 (the crossover definition).
    #[test]
    fn residual_is_half_at_crossover() {
        assert!((migration_residual(2.0, 2.0) - 0.5).abs() < 1e-6);
        assert!((migration_residual(0.5, 0.5) - 0.5).abs() < 1e-6);
    }

    // Metamorphic: residual is strictly decreasing in natural_strength.
    // A stronger natural drive must always attenuate migration more.
    #[test]
    fn residual_strictly_decreasing_in_natural_strength() {
        let quiet = 1.0;
        let r_low = migration_residual(0.5, quiet);
        let r_mid = migration_residual(1.0, quiet);
        let r_high = migration_residual(5.0, quiet);
        assert!(r_low > r_mid, "residual must fall as natural_strength rises");
        assert!(r_mid > r_high);
    }

    // Residual tends toward near-zero for very large natural_strength.
    #[test]
    fn residual_vanishes_for_strong_naturals() {
        assert!(migration_residual(1000.0, 1.0) < 0.01);
    }

    // ── Drives::total and migration_share ─────────────────────────────────────

    // Sanity: total() equals the component sum on hand-picked inputs.
    #[test]
    fn drives_total_is_component_sum() {
        let d = Drives {
            sep: Vec2::new(1.0, 0.0),
            coh: Vec2::new(0.0, 1.0),
            grass: Vec2::new(-0.5, 0.0),
            social: Vec2::ZERO,
            migration: Vec2::new(0.3, 0.0),
        };
        let expected = Vec2::new(0.8, 1.0);
        assert!((d.total() - expected).length() < 1e-6);
    }

    // migration_share is 0 when all drives are zero.
    #[test]
    fn migration_share_is_zero_when_idle() {
        let d = Drives {
            sep: Vec2::ZERO,
            coh: Vec2::ZERO,
            grass: Vec2::ZERO,
            social: Vec2::ZERO,
            migration: Vec2::ZERO,
        };
        assert_eq!(d.migration_share(), 0.0);
    }

    // ── combine_drives (metamorphic) ─────────────────────────────────────────

    // Metamorphic: raising params.migration raises migration_share.
    // A bigger migration weight must contribute a larger fraction of total pull.
    #[test]
    fn migration_share_rises_with_migration_param() {
        let mut p_lo = default_params();
        p_lo.migration = 0.1;
        let mut p_hi = default_params();
        p_hi.migration = 1.5;

        let sep = Vec2::new(1.0, 0.5);
        let coh = Vec2::new(-0.5, 1.0);
        let grass = Vec2::new(0.3, 0.8);
        let social = Vec2::ZERO;
        let pressure = 0.8;
        let energy = 0.5;

        let lo = combine_drives(sep, coh, grass, social, &p_lo, pressure, energy);
        let hi = combine_drives(sep, coh, grass, social, &p_hi, pressure, energy);
        assert!(
            hi.migration_share() > lo.migration_share(),
            "higher migration param must raise migration_share"
        );
    }

    // Metamorphic: raising pressure raises migration_share.
    // Higher pack pressure amplifies the migration term.
    #[test]
    fn migration_share_rises_with_pressure() {
        let p = default_params();
        let sep = Vec2::new(1.0, 0.0);
        let coh = Vec2::new(0.0, 1.0);
        let grass = Vec2::new(0.5, 0.5);
        let social = Vec2::ZERO;
        let energy = 0.5;

        let lo = combine_drives(sep, coh, grass, social, &p, 0.1, energy);
        let hi = combine_drives(sep, coh, grass, social, &p, 1.0, energy);
        assert!(
            hi.migration_share() > lo.migration_share(),
            "higher pressure must raise migration_share"
        );
    }

    // Metamorphic: stronger natural weights reduce migration_share via the residual.
    // Scaling up separation, cohesion, grass, and social weights shrinks migration's share
    // because the residual attenuates it. This replaces the old constant-migration relation.
    #[test]
    fn migration_share_falls_as_natural_weights_grow() {
        let mut p_weak = default_params();
        p_weak.separation = 0.1;
        p_weak.cohesion = 0.1;
        p_weak.grass = 0.1;
        p_weak.social = 0.1;

        let mut p_strong = default_params();
        p_strong.separation = 3.0;
        p_strong.cohesion = 3.0;
        p_strong.grass = 3.0;
        p_strong.social = 3.0;

        let sep = Vec2::new(1.0, 0.5);
        let coh = Vec2::new(-0.5, 1.0);
        let grass = Vec2::new(0.3, 0.8);
        let social = Vec2::new(0.2, 0.1);
        let pressure = 1.0;
        let energy = 0.5;

        let weak = combine_drives(sep, coh, grass, social, &p_weak, pressure, energy);
        let strong = combine_drives(sep, coh, grass, social, &p_strong, pressure, energy);
        assert!(
            weak.migration_share() > strong.migration_share(),
            "stronger natural drives must reduce migration_share via residual"
        );
    }

    // Metamorphic: a hungrier elk (lower energy) raises the grass+social share.
    // Hunger sharpens appetite, amplifying food drives relative to migration.
    #[test]
    fn hunger_raises_food_drive_share() {
        let p = default_params();
        let sep = Vec2::new(1.0, 0.0);
        let coh = Vec2::new(0.0, 1.0);
        let grass = Vec2::new(0.5, 0.5);
        let social = Vec2::new(0.3, 0.0);
        let pressure = 1.0;

        let well_fed = combine_drives(sep, coh, grass, social, &p, pressure, 1.0); // energy=1
        let starving = combine_drives(sep, coh, grass, social, &p, pressure, 0.0); // energy=0

        let food_share_fed = (well_fed.grass.length() + well_fed.social.length())
            / (well_fed.sep.length()
                + well_fed.coh.length()
                + well_fed.grass.length()
                + well_fed.social.length()
                + well_fed.migration.length());
        let food_share_starving = (starving.grass.length() + starving.social.length())
            / (starving.sep.length()
                + starving.coh.length()
                + starving.grass.length()
                + starving.social.length()
                + starving.migration.length());

        assert!(
            food_share_starving > food_share_fed,
            "hunger must raise the food-drive share"
        );
    }

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
