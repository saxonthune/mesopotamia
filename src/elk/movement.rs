use bevy::prelude::*;
use rand::Rng;

use crate::grid::Grid;

use super::components::{Elk, ElkParams, Packs};

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

/// Two-phase boids-on-a-lattice. Phase 1 snapshots every elk's position and
/// pack; phase 2 builds a desire vector from four *normalized* drives —
/// separation, cohesion, grass-gradient, and the migration fallback — then
/// picks a step by softmax weighted-random.
pub(super) fn herd_move(
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
