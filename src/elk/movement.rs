use bevy::prelude::*;
use rand::Rng;

use crate::grid::Grid;

use super::components::{DriveSample, DriveSamples, Elk, ElkParams, HabitatIntake, LastDecision, Packs, ProbeSeed};
use super::ledger::EnergyFlows;

const OTHER_PACK_SEP: f32 = 0.5; // mild push from foreign packs

/// Labeled view of a weighted decomposition — each entry is (label, vector).
pub trait Decomposable {
    fn contributions(&self) -> Vec<(&'static str, Vec2)>;
}

/// One candidate step evaluated by the softmax decision.
pub struct StepEval {
    pub step: (isize, isize),
    pub score: f32,
    pub penalty: f32,
    pub weight: f32,
}

/// Full per-elk, per-tick decision record captured by `herd_move`.
pub struct Decision {
    pub drives: Drives,
    pub options: Vec<StepEval>,
    /// Index into `options` of the chosen step; None when hemmed in.
    pub chosen: Option<usize>,
    pub temperature: f32,
}

impl Default for Decision {
    fn default() -> Self {
        Decision {
            drives: Drives {
                sep: Vec2::ZERO,
                coh: Vec2::ZERO,
                grass: Vec2::ZERO,
                social: Vec2::ZERO,
                migration: Vec2::ZERO,
            },
            options: Vec::new(),
            chosen: None,
            temperature: 0.0,
        }
    }
}

/// Step cost for entering a water cell. On a ford the cost is reduced by
/// `ford_discount`; off a ford it is the raw water-level × cost (today's behaviour).
pub fn step_water_penalty(water: f32, is_ford: bool, water_cost: f32, ford_discount: f32) -> f32 {
    let base = water * water_cost;
    if is_ford { base * ford_discount } else { base }
}

/// Grass-gradient field at `cell`: the pull-toward-forage vector that `herd_move`
/// steers by, as a pure function of the grid and params. Nearby rich cells weigh
/// more; cells beyond `grass_radius` are ignored. Returns a raw (un-normalized)
/// direction vector — magnitude reflects how strongly forage pulls from each direction.
pub fn grass_gradient(cell: usize, grid: &Grid, params: &ElkParams) -> Vec2 {
    let gr = params.grass_radius.ceil() as isize;
    let mut dir = Vec2::ZERO;
    for dy in -gr..=gr {
        for dx in -gr..=gr {
            if dx == 0 && dy == 0 {
                continue;
            }
            if let Some(n) = grid.step(cell, dx, dy) {
                let off = Vec2::new(dx as f32, dy as f32);
                let dist = off.length();
                if dist > params.grass_radius {
                    continue;
                }
                dir += off / dist * (grid.forage(n) / dist);
            }
        }
    }
    dir
}

/// Water-penalty field at `cell`: the step cost an elk would pay to enter this
/// cell, sampled for overlay rendering. Thin wrapper so grid sampling does not
/// have to repeat the ford/cost arithmetic.
pub fn cell_water_penalty(cell: usize, grid: &Grid, params: &ElkParams) -> f32 {
    step_water_penalty(grid.water(cell), grid.is_ford(cell), params.water_cost, params.ford_discount)
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

/// Patch-leaving gate: how strongly an elk should stay on the current patch vs.
/// leave for extensive search. Returns a scale in [0, 1] — 1.0 means stay
/// (intensive local search); 0.0 means leave (extensive directional movement).
///
/// Form: linear ramp from 0 at `giving_up · habitat_mean` to 1 at `habitat_mean`,
/// clamped to [0, 1]:
///
///   gate = clamp((local_intake − giving_up·mean) / (mean·(1 − giving_up)), 0, 1)
///
/// When `habitat_mean ≤ 0` (no signal yet) the gate returns 1.0 — no suppression.
pub fn forage_gate(local_intake: f32, habitat_mean: f32, giving_up: f32) -> f32 {
    if habitat_mean <= 0.0 {
        return 1.0;
    }
    let denom = habitat_mean * (1.0 - giving_up);
    if denom <= 0.0 {
        return 1.0;
    }
    ((local_intake - giving_up * habitat_mean) / denom).clamp(0.0, 1.0)
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

impl Decomposable for Drives {
    fn contributions(&self) -> Vec<(&'static str, Vec2)> {
        vec![
            ("sep",       self.sep),
            ("coh",       self.coh),
            ("grass",     self.grass),
            ("social",    self.social),
            ("migration", self.migration),
        ]
    }
}

/// Combine the raw per-drive direction accumulators into weighted contributions.
/// `sep`/`coh`/`grass_dir`/`social` are the un-normalized accumulators built in
/// `herd_move`; `pressure` is the pack's migration pressure; `energy` is the elk's
/// energy (hunger sharpens the food drives). `gate` is the patch-leaving scale
/// from `forage_gate` — it suppresses grass/social when the elk is below the
/// habitat average, and amplifies migration to push it into extensive search.
/// Migration is residual — it scales by how quiet the naturals are.
pub fn combine_drives(
    sep: Vec2,
    coh: Vec2,
    grass_dir: Vec2,
    social: Vec2,
    params: &ElkParams,
    pressure: f32,
    energy: f32,
    gate: f32,
) -> Drives {
    let hunger = 1.0 - energy;
    let appetite = 0.25 + 0.75 * hunger;
    let sep = norm(sep) * params.separation;
    let coh = norm(coh) * params.cohesion;
    let grass = norm(grass_dir) * (params.grass * appetite * gate);
    let social = norm(social) * (params.social * appetite * gate);
    let natural_strength = sep.length() + coh.length() + grass.length() + social.length();
    let mig_boost = 1.0 + params.leave_boost * (1.0 - gate);
    let migration = Vec2::X
        * (params.migration * pressure * migration_residual(natural_strength, params.quiet) * mig_boost);
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
    mut elk_q: Query<(&mut Elk, &mut LastDecision)>,
    mut samples: ResMut<DriveSamples>,
    mut flows: ResMut<EnergyFlows>,
    mut probe_seed: Option<ResMut<ProbeSeed>>,
    habitat_intake: Res<HabitatIntake>,
) {
    // Phase 1: snapshot (col, row, slot, grazing) for every elk, in query order.
    let snapshot: Vec<(f32, f32, u8, bool)> = elk_q
        .iter()
        .map(|(elk, _)| {
            let (col, row) = grid.col_row(elk.cell);
            (col as f32, row as f32, elk.slot, elk.grazing)
        })
        .collect();

    // Phase 2: decide and write, reading neighbours only from the snapshot.
    let mut acc = vec![DriveSample::default(); super::PACK_COUNT];
    let steps: [(isize, isize); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];

    for (i, (mut elk, mut last_decision)) in elk_q.iter_mut().enumerate() {
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

        // Field drive: steer up the grass gradient. Extracted to `grass_gradient`
        // so the same field can be sampled on arbitrary cells for the overlay.
        let grass_dir = grass_gradient(elk.cell, &grid, &params);

        // Combine normalized drives. Hunger sharpens the pull toward food —
        // both grass directly and other elk already feeding — so a fed herd
        // drifts while a starving one bolts toward the nearest feast.
        // Migration is a residual: it fills in only as the natural drives fall quiet.
        // The patch-leaving gate suppresses grass/social for below-average foragers
        // and amplifies migration, nudging them into extensive search.
        let gate = forage_gate(elk.intake_rate, habitat_intake.mean, params.giving_up);
        let drives = combine_drives(
            sep, coh, grass_dir, social, &params, packs.migration[slot as usize], elk.energy, gate,
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
        // Move drives into a local so it can be placed into the Decision record.
        let decision_drives = drives;

        // Hunger sharpens the urge to ford (matches the appetite in combine_drives:
        // a fed elk has no reason to risk the river), and the forage under the elk
        // now is the baseline a crossing must beat.
        let appetite = 0.25 + 0.75 * (1.0 - elk.energy);
        let here_forage = grid.forage(elk.cell);

        // Score each valid step, then softmax for a weighted-random pick.
        let mut scores = [f32::NEG_INFINITY; 4];
        let mut cells: [Option<usize>; 4] = [None; 4];
        let mut penalties = [0.0_f32; 4];
        for (k, &(dx, dy)) in steps.iter().enumerate() {
            if let Some(next) = grid.step(elk.cell, dx, dy) {
                // Fording is costly — deep water repels, a ford less so. This
                // is what turns a crossing into a decision: a herd only steps into
                // water when the forage drive beyond outweighs the penalty.
                let penalty = step_water_penalty(
                    grid.water(next),
                    grid.is_ford(next),
                    params.water_cost,
                    params.ford_discount,
                );
                let mut score = desire.dot(Vec2::new(dx as f32, dy as f32)) - penalty;
                // Crossing incentive: when the step enters water, peek across for
                // the far bank and add `cross_desire` — the forage gain over the
                // best dry option, net of the whole span's cost. Scaled by hunger
                // and the tunable `cross` weight, this is what lets a starving herd
                // commit to a ford toward grass it cannot otherwise sense.
                if grid.water(next) >= WATER_EPS {
                    if let Some((across, span_cost)) = forage_across(
                        &grid, elk.cell, dx, dy, params.water_cost, params.ford_discount, MAX_PEEK,
                    ) {
                        let ahead = grid.forage(next);
                        let incentive = cross_desire(here_forage, ahead, across, span_cost).max(0.0);
                        score += params.cross * appetite * incentive;
                    }
                }
                scores[k] = score;
                penalties[k] = penalty;
                cells[k] = Some(next);
            }
        }
        let max = scores.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        if !max.is_finite() {
            // Hemmed in — write an empty Decision so the UI can show the state.
            last_decision.0 = Decision {
                drives: decision_drives,
                options: Vec::new(),
                chosen: None,
                temperature: params.temperature,
            };
            continue;
        }
        let mut weights = [0.0_f32; 4];
        for k in 0..4 {
            if scores[k].is_finite() {
                weights[k] = ((scores[k] - max) / params.temperature).exp();
            }
        }

        // Build the per-candidate record before the pick so chosen maps into options.
        let mut options = Vec::with_capacity(4);
        for (k, &(dx, dy)) in steps.iter().enumerate() {
            if scores[k].is_finite() {
                options.push(StepEval {
                    step: (dx, dy),
                    score: scores[k],
                    penalty: penalties[k],
                    weight: weights[k],
                });
            }
        }

        // The max-scored step always has weight 1, so total >= 1 (no div-by-0).
        let total: f32 = weights.iter().sum();
        // Use the persistent probe RNG (same state across ticks) for
        // determinism, or the thread RNG for normal simulation runs.
        let mut pick = if let Some(ref mut ps) = probe_seed {
            ps.rng().random_range(0.0..total)
        } else {
            rand::rng().random_range(0.0..total)
        };
        let mut option_counter = 0usize;
        let mut chosen_k: Option<usize> = None;
        for k in 0..4 {
            let Some(next) = cells[k] else { continue };
            if pick < weights[k] {
                elk.cell = next;
                chosen_k = Some(option_counter);
                break;
            }
            pick -= weights[k];
            option_counter += 1;
        }

        last_decision.0 = Decision {
            drives: decision_drives,
            options,
            chosen: chosen_k,
            temperature: params.temperature,
        };

        // Swim energy cost: crossing deep non-ford water is a lasting risk beyond
        // the step-score penalty — the elk arrives tired.
        let water = grid.water(elk.cell);
        if water > 0.0 && !grid.is_ford(elk.cell) {
            let before_swim = elk.energy;
            elk.energy = (elk.energy - params.swim_drain * water).max(0.0);
            flows.swim += before_swim - elk.energy;
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
pub fn cross_desire(here: f32, ahead: f32, across: f32, cross_cost: f32) -> f32 {
    across - here.max(ahead) - cross_cost
}

/// A water step entering a cell with at least this water level engages the
/// crossing incentive; below it the step is dry land and scored normally.
const WATER_EPS: f32 = 0.01;

/// How far across a water span an elk looks for the far bank. Bounds the peek so
/// a step parallel to a long river (which never reaches dry land) stops cheaply.
const MAX_PEEK: usize = 12;

/// Look across a water span for the far bank `cross_desire` would aim at. Walks
/// from `cell` in `(dx, dy)`, summing the per-cell crossing penalty over the
/// contiguous water, and returns `(far_bank_forage, span_cost)` at the first dry
/// cell reached within `max_peek` steps. `None` when the step does not enter
/// water, the water never ends within reach, or the path runs off the grid.
///
/// This is what lets a hungry herd "see" greener ground beyond a river it cannot
/// otherwise perceive (the far bank sits past the grass-gradient radius), so the
/// barrier becomes a decision instead of a wall.
pub fn forage_across(
    grid: &Grid,
    cell: usize,
    dx: isize,
    dy: isize,
    water_cost: f32,
    ford_discount: f32,
    max_peek: usize,
) -> Option<(f32, f32)> {
    let mut span_cost = 0.0;
    let mut at = cell;
    let mut crossed_water = false;
    for _ in 0..max_peek {
        let next = grid.step(at, dx, dy)?;
        let water = grid.water(next);
        if water < WATER_EPS {
            // Dry cell: the far bank — but only if we actually crossed water.
            return crossed_water.then(|| (grid.forage(next), span_cost));
        }
        span_cost += step_water_penalty(water, grid.is_ford(next), water_cost, ford_discount);
        crossed_water = true;
        at = next;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::components::DriveSample;

    // ── Decomposable ─────────────────────────────────────────────────────────

    // Labels returned by contributions() must match DRIVE_COLORS order in ui.rs:
    // sep, coh, grass, social, migration.
    #[test]
    fn decomposable_labels_match_drive_colors_order() {
        let d = Drives {
            sep: Vec2::X, coh: Vec2::Y, grass: Vec2::X, social: Vec2::Y, migration: Vec2::X,
        };
        let labels: Vec<&str> = d.contributions().iter().map(|(l, _)| *l).collect();
        assert_eq!(labels, ["sep", "coh", "grass", "social", "migration"]);
    }

    // total() must equal the vector sum of contributions().
    #[test]
    fn decomposable_sum_equals_total() {
        let d = Drives {
            sep: Vec2::new(1.0, 0.0),
            coh: Vec2::new(0.0, 1.0),
            grass: Vec2::new(-0.5, 0.0),
            social: Vec2::ZERO,
            migration: Vec2::new(0.3, 0.0),
        };
        let sum = d.contributions().into_iter().map(|(_, v)| v).fold(Vec2::ZERO, |a, b| a + b);
        assert!((sum - d.total()).length() < 1e-6);
    }

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

        let lo = combine_drives(sep, coh, grass, social, &p_lo, pressure, energy, 1.0);
        let hi = combine_drives(sep, coh, grass, social, &p_hi, pressure, energy, 1.0);
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

        let lo = combine_drives(sep, coh, grass, social, &p, 0.1, energy, 1.0);
        let hi = combine_drives(sep, coh, grass, social, &p, 1.0, energy, 1.0);
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

        let weak = combine_drives(sep, coh, grass, social, &p_weak, pressure, energy, 1.0);
        let strong = combine_drives(sep, coh, grass, social, &p_strong, pressure, energy, 1.0);
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

        let well_fed = combine_drives(sep, coh, grass, social, &p, pressure, 1.0, 1.0); // energy=1
        let starving = combine_drives(sep, coh, grass, social, &p, pressure, 0.0, 1.0); // energy=0

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

    // ── grass_gradient ────────────────────────────────────────────────────────

    fn flat_grid(w: usize, h: usize, forage: f32) -> super::super::super::grid::Grid {
        let mut g = crate::grid::Grid::new(w, h);
        for i in 0..g.len() {
            g.set_grass(i, forage);
        }
        g
    }

    // A fully uniform forage field has zero gradient — no direction to pull.
    // Grid is 21x21 so the center cell's full radius-5 neighbourhood is in bounds.
    #[test]
    fn gradient_zero_on_flat_forage() {
        let grid = flat_grid(21, 21, 0.5);
        let params = default_params();
        let center = 10 * 21 + 10;
        let g = grass_gradient(center, &grid, &params);
        assert!(g.length() < 1e-4, "flat forage must yield zero gradient, got {g:?}");
    }

    // The gradient points toward the richer cell (+x direction).
    #[test]
    fn gradient_points_toward_richer_forage() {
        let mut grid = crate::grid::Grid::new(21, 21);
        let center = 10 * 21 + 10;
        let right = center + 1; // col+1, same row
        grid.set_grass(right, 1.0);
        let params = default_params();
        let g = grass_gradient(center, &grid, &params);
        assert!(g.x > 0.0, "gradient must point toward richer forage (+x), got {g:?}");
        assert!(g.x.abs() > g.y.abs(), "gradient must be predominantly rightward");
    }

    // Raising a neighbour's forage raises the gradient magnitude (metamorphic).
    #[test]
    fn gradient_magnitude_rises_with_neighbor_forage() {
        let mut grid = crate::grid::Grid::new(21, 21);
        let center = 10 * 21 + 10;
        let right = center + 1;
        let params = default_params();

        grid.set_grass(right, 0.3);
        let low = grass_gradient(center, &grid, &params).length();

        grid.set_grass(right, 0.9);
        let high = grass_gradient(center, &grid, &params).length();

        assert!(high > low, "richer forage must raise gradient magnitude");
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

    // ── forage_across ─────────────────────────────────────────────────────────

    /// A row world: dry start (0), two water cells (1,2), dry far bank (3) with grass.
    fn river_row() -> Grid {
        let mut grid = Grid::new(6, 1);
        grid.set_water(1, 1.0);
        grid.set_water(2, 1.0);
        grid.set_grass(3, 0.5);
        grid
    }

    #[test]
    fn forage_across_finds_far_bank_and_sums_span_cost() {
        let grid = river_row();
        // Two non-ford water cells at water_cost 2.0 ⇒ span cost 4.0; far bank forage 0.5.
        let (across, cost) = forage_across(&grid, 0, 1, 0, 2.0, 0.1, 12).unwrap();
        assert!((across - 0.5).abs() < 1e-6);
        assert!((cost - 4.0).abs() < 1e-6);
    }

    #[test]
    fn forage_across_is_none_when_step_is_dry() {
        // Stepping the other way (−x off cell 0) and into dry land yields no crossing.
        let mut grid = Grid::new(6, 1);
        grid.set_grass(1, 0.5); // dry neighbour to the +x side
        assert!(forage_across(&grid, 0, 1, 0, 2.0, 0.1, 12).is_none());
    }

    #[test]
    fn forage_across_is_none_when_far_bank_out_of_reach() {
        let mut grid = Grid::new(6, 1);
        for i in 1..6 {
            grid.set_water(i, 1.0); // water all the way to the edge — no far bank
        }
        assert!(forage_across(&grid, 0, 1, 0, 2.0, 0.1, 12).is_none());
    }

    #[test]
    fn forage_across_charges_less_over_a_ford() {
        let mut grid = river_row();
        grid.set_ford(1, true);
        grid.set_ford(2, true);
        // ford_discount 0.1 ⇒ each water cell costs 0.2 instead of 2.0.
        let (_, cost) = forage_across(&grid, 0, 1, 0, 2.0, 0.1, 12).unwrap();
        assert!((cost - 0.4).abs() < 1e-6, "ford span should cost 2×0.2 = 0.4, got {cost}");
    }

    // ── forage_gate ───────────────────────────────────────────────────────────

    // Gate is 1.0 when local intake equals habitat mean (at-average → stay).
    #[test]
    fn forage_gate_is_one_at_habitat_mean() {
        assert!((forage_gate(0.5, 0.5, 0.6) - 1.0).abs() < 1e-6);
    }

    // Gate is 1.0 when local intake exceeds habitat mean.
    #[test]
    fn forage_gate_is_one_above_habitat_mean() {
        assert!((forage_gate(0.8, 0.5, 0.6) - 1.0).abs() < 1e-6);
    }

    // Gate is 0.0 at or below the giving_up floor.
    #[test]
    fn forage_gate_is_zero_at_giving_up_floor() {
        let mean = 0.5;
        let giving_up = 0.6;
        let floor = giving_up * mean;
        assert!((forage_gate(floor, mean, giving_up)).abs() < 1e-6);
        assert!((forage_gate(floor - 0.1, mean, giving_up)).abs() < 1e-6);
    }

    // Gate is strictly bounded in [0, 1].
    #[test]
    fn forage_gate_bounded() {
        for local in [0.0, 0.1, 0.3, 0.5, 0.7, 1.0] {
            let g = forage_gate(local, 0.4, 0.6);
            assert!((0.0..=1.0).contains(&g), "gate out of bounds: {g}");
        }
    }

    // Gate is non-increasing as local_intake falls.
    #[test]
    fn forage_gate_monotone_decreasing_in_local_intake() {
        let mean = 0.5;
        let giving_up = 0.6;
        let hi = forage_gate(0.5, mean, giving_up);
        let mid = forage_gate(0.35, mean, giving_up);
        let lo = forage_gate(0.1, mean, giving_up);
        assert!(hi >= mid, "gate must not rise as local falls");
        assert!(mid >= lo, "gate must not rise as local falls");
    }

    // Gate returns 1.0 when habitat_mean is zero (no signal → no suppression).
    #[test]
    fn forage_gate_is_one_when_mean_is_zero() {
        assert!((forage_gate(0.0, 0.0, 0.6) - 1.0).abs() < 1e-6);
        assert!((forage_gate(0.5, 0.0, 0.6) - 1.0).abs() < 1e-6);
    }

    // Gate returns 1.0 when habitat_mean is negative.
    #[test]
    fn forage_gate_is_one_when_mean_negative() {
        assert!((forage_gate(0.0, -1.0, 0.6) - 1.0).abs() < 1e-6);
    }
}
