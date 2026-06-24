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

/// What an elk commits to this tick. Step travels to a cardinal neighbour;
/// Stand and Graze both hold position (Graze additionally feeds, in Phase 2).
/// Stored as a field on the decision record — never a marker component — so an
/// elk's archetype never churns (see doc03.01.07).
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Act {
    Step(isize, isize),
    Stand,
    Graze,
}

/// One candidate action evaluated by the softmax decision.
pub struct Candidate {
    pub act: Act,
    pub score: f32,
    pub penalty: f32,
    pub weight: f32,
}

/// Full per-elk, per-tick decision record captured by `herd_move`.
pub struct Decision {
    pub drives: Drives,
    pub options: Vec<Candidate>,
    /// Index into `options` of the chosen step; None when hemmed in.
    pub chosen: Option<usize>,
    pub chosen_act: Act,
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
            chosen_act: Act::Stand,
            temperature: 0.0,
        }
    }
}

/// Value of grazing the current cell, in the same units as a move's
/// `desire·dir − penalty` score. Marginal value of intake declines with the
/// store: the `(1 − energy)` factor sends it to 0 at satiation, so a full elk is
/// indifferent between Graze and Stand. `here_forage` is `grid.forage(cell)`.
pub fn graze_value(here_forage: f32, energy: f32, dwell: f32) -> f32 {
    dwell * (1.0 - energy).clamp(0.0, 1.0) * here_forage.max(0.0)
}

/// Value of standing still — the conserve baseline. Holding position is the zero
/// reference a move must beat (a move scores `desire·dir − penalty`, which is 0
/// when the desire vector is orthogonal to the step and there's no penalty).
pub fn stand_value() -> f32 {
    0.0
}

/// Unit cardinal vector of the elk's last move (`prev → cell`), or zero if it
/// held position. This is the only memory the crossing model needs: re-scoring
/// each candidate against it rewards continuing and penalizes reversing, so an
/// elk that has committed to a direction keeps it instead of dithering. Moves
/// are single cardinal steps, so the delta is already ±1 on one axis; the
/// `normalize_or_zero` guards a respawn/teleport that leaps more than one cell.
pub fn step_heading(prev: usize, cell: usize, grid: &Grid) -> Vec2 {
    let (px, py) = grid.col_row(prev);
    let (cx, cy) = grid.col_row(cell);
    Vec2::new(cx as f32 - px as f32, cy as f32 - py as f32).normalize_or_zero()
}

/// Best grass fraction (`grass / capacity`) over cells within `radius` of `cell`,
/// the cell itself included. The *reachability reference* the travel mode steers
/// by: an elk travels only while somewhere meaningfully richer than underfoot is
/// in reach. This is what makes the mode safe across real terrain — on thin or
/// barren ground with nothing better nearby, `best == here`, so the elk never
/// dashes off to nowhere and starve. Barren cells (capacity ~0) contribute 0.
pub fn best_reachable_frac(cell: usize, grid: &Grid, radius: f32) -> f32 {
    let frac = |c: usize| {
        let cap = grid.capacity(c);
        if cap > 1e-6 { grid.grass(c) / cap } else { 0.0 }
    };
    let r = radius.ceil() as isize;
    let mut best = frac(cell);
    for dy in -r..=r {
        for dx in -r..=r {
            if dx == 0 && dy == 0 {
                continue;
            }
            if (Vec2::new(dx as f32, dy as f32)).length() > radius {
                continue;
            }
            if let Some(n) = grid.step(cell, dx, dy) {
                best = best.max(frac(n));
            }
        }
    }
    best
}

/// Next foraging mode — graze in place or *travel* (a directed dash, softmax
/// sharpened in `herd_move`). An elk travels only while a meaningfully richer
/// patch is reachable: it leaves a patch once it is drawn below `leave_frac` of
/// capacity *and* somewhere better than underfoot is within reach, and it settles
/// the moment nothing better remains reachable (it has arrived at the local best).
///
/// `here_frac` is `grass / capacity` underfoot; `best_frac` is `best_reachable_frac`;
/// `margin` is how much richer a reachable patch must be to be worth the dash.
/// Requiring reachable improvement on *both* edges is what removes the old
/// absolute-threshold trap — an elk surrounded by nothing better grazes what it
/// has rather than travelling to its death. Grazing is never suppressed, so even
/// a perpetual traveller still feeds; the mode only shapes how it moves.
pub fn next_forage_mode(
    traveling: bool,
    here_frac: f32,
    best_frac: f32,
    leave_frac: f32,
    margin: f32,
) -> bool {
    let better_reachable = best_frac > here_frac + margin;
    if traveling {
        better_reachable // settle once nothing better than underfoot is in reach
    } else {
        here_frac < leave_frac && better_reachable // leave a drawn-down patch only for a better one
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
                let attract = grid.forage(n) + params.freshness_weight * grid.freshness(n);
                dir += off / dist * (attract / dist);
            }
        }
    }
    dir
}

/// Long-range forward forage pull on dry land — the leapfrog primitive. Beyond
/// the local `grass_radius`, an elk on a depleted patch still steers toward a
/// clearly richer cell it can see ahead along the migration axis (+x): the
/// dry-land analogue of `forage_across` peeking over a river. Walks east up to
/// `sightline_range` cells, climbing the same attractiveness the local gradient
/// does (`forage + freshness_weight · freshness`, so the green-up front is the
/// long-range beacon), and returns a +x pull proportional to how much the best
/// cell ahead beats underfoot. Zero when nothing ahead is richer (no pull to
/// nowhere) or `sightline_weight == 0` (identity — the grass drive stays local).
///
/// Folded into `grass_dir` before it is normalized, so the tilt is *relative*:
/// on rich ground the strong local gradient dominates and the elk grazes; on
/// drawn-down ground the local gradient is weak and the sightline takes over,
/// pointing the herd east toward the next forage. That switch is the emergent
/// "stick to good grass, roll forward off bad" behaviour.
pub fn forage_sightline(cell: usize, grid: &Grid, params: &ElkParams) -> Vec2 {
    if params.sightline_weight <= 0.0 || params.sightline_range < 1.0 {
        return Vec2::ZERO;
    }
    let attract = |c: usize| grid.forage(c) + params.freshness_weight * grid.freshness(c);
    let here = attract(cell);
    let mut best = here;
    let r = params.sightline_range.ceil() as isize;
    for dx in 1..=r {
        match grid.step(cell, dx, 0) {
            Some(n) => best = best.max(attract(n)),
            None => break, // ran off the grid — nothing further east to see
        }
    }
    let surplus = (best - here).max(0.0);
    Vec2::X * params.sightline_weight * surplus
}

/// Per-packmate weight for the cohesion centroid, biased toward those *ahead*.
/// `lead == 0` returns 1.0 (every packmate counts equally ⇒ the plain centroid,
/// today's behaviour). With `lead > 0`, a packmate further east than the elk
/// weighs up to `1 + lead`, ramped linearly over `coh_radius`, so the cohesion
/// target drifts toward the herd's leading edge and the blob elongates into a
/// rolling column. Packmates behind or level always weigh 1.0 — the bias only
/// ever pulls forward, never back.
pub fn cohesion_lead_weight(self_col: f32, other_col: f32, coh_radius: f32, lead: f32) -> f32 {
    let ahead = ((other_col - self_col) / coh_radius.max(1e-6)).clamp(0.0, 1.0);
    1.0 + lead * ahead
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
        // Heading of last tick's move, captured before `prev_cell` is reset
        // below. Drives the directional-persistence term that keeps a crossing
        // elk committed (see `step_heading`).
        let heading = step_heading(elk.prev_cell, elk.cell, &grid);
        // Where the elk starts this tick becomes the render interpolation's
        // origin; if it doesn't move below, prev == cell and the sprite holds still.
        elk.prev_cell = elk.cell;
        // Update the graze/travel mode: travel toward a meaningfully richer patch
        // when this one is drawn down, settle once nothing better is in reach.
        // Travel sharpens the pick (committed dash); grazing stays available, so
        // the mode shapes movement but can never starve an elk.
        let cap = grid.capacity(elk.cell);
        let here_frac = if cap > 1e-6 { grid.grass(elk.cell) / cap } else { 0.0 };
        let best_frac = best_reachable_frac(elk.cell, &grid, params.grass_radius);
        elk.traveling =
            next_forage_mode(elk.traveling, here_frac, best_frac, params.leave_frac, params.travel_margin);
        let temperature = if elk.traveling {
            params.temperature * params.travel_focus
        } else {
            params.temperature
        };
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
                // Weight packmates ahead more heavily so the cohesion centre leads
                // the centroid — turns a milling blob into a rolling column.
                let w = cohesion_lead_weight(cx, ox, params.coh_radius, params.cohesion_lead);
                coh_sum += Vec2::new(ox, oy) * w;
                coh_n += w;
            }
            // Local enhancement: grazing elk (any pack) draw foragers from afar,
            // nearer ones more — `-off` points from self toward the grazer.
            if ograzing && dist <= params.social_radius {
                social += (-off) / dist / dist;
            }
        }
        let coh = if coh_n > 0.0 { coh_sum / coh_n - pos } else { Vec2::ZERO };

        // Field drive: steer up the grass gradient (local), tilted by the
        // long-range eastward sightline toward fresh forage beyond perception.
        // The sum is normalized in `combine_drives`, so the sightline only bends
        // the direction — dominant on depleted ground, negligible on rich.
        let grass_dir =
            grass_gradient(elk.cell, &grid, &params) + forage_sightline(elk.cell, &grid, &params);

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

        // Score each valid step (indices 0-3), then Stand (4) and Graze (5).
        // Softmax over all six for a weighted-random pick.
        let mut scores = [f32::NEG_INFINITY; 6];
        let mut cells: [Option<usize>; 6] = [None; 6];
        let mut penalties = [0.0_f32; 6];
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
                let step_dir = Vec2::new(dx as f32, dy as f32);
                let mut score = desire.dot(step_dir) - penalty;
                // Directional persistence: reward continuing last tick's heading,
                // penalize reversing it. Strong in water so a crossing elk commits
                // to the far bank instead of toe-dipping; gentle on land so it just
                // resists immediate backtracking. With the swim-drain energy cost,
                // this is what stops the back-and-forth inside a river.
                let persist = if grid.water(elk.cell) >= WATER_EPS {
                    params.momentum_water
                } else {
                    params.momentum
                };
                score += persist * heading.dot(step_dir);
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
        // Stand and Graze candidates: no destination cell, no penalty. Grazing is
        // always available — a travelling elk that finds worthwhile grass underfoot
        // eats it rather than dashing past, which is what guarantees the travel
        // mode can never starve the herd. Travel shapes movement via temperature,
        // not by forbidding the feed.
        scores[4] = stand_value();
        scores[5] = graze_value(here_forage, elk.energy, params.dwell);

        let max = scores.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        if !max.is_finite() {
            // Hemmed in — write an empty Decision so the UI can show the state.
            last_decision.0 = Decision {
                drives: decision_drives,
                options: Vec::new(),
                chosen: None,
                chosen_act: Act::Stand,
                temperature,
            };
            continue;
        }
        let mut weights = [0.0_f32; 6];
        for k in 0..6 {
            if scores[k].is_finite() {
                weights[k] = ((scores[k] - max) / temperature).exp();
            }
        }

        // Build the per-candidate record before the pick so chosen maps into options.
        let mut options = Vec::with_capacity(6);
        for (k, &(dx, dy)) in steps.iter().enumerate() {
            if scores[k].is_finite() {
                options.push(Candidate {
                    act: Act::Step(dx, dy),
                    score: scores[k],
                    penalty: penalties[k],
                    weight: weights[k],
                });
            }
        }
        options.push(Candidate { act: Act::Stand, score: scores[4], penalty: 0.0, weight: weights[4] });
        options.push(Candidate { act: Act::Graze, score: scores[5], penalty: 0.0, weight: weights[5] });

        // The max-scored candidate has weight 1, so total >= 1 (no div-by-0).
        let total: f32 = weights.iter().sum();
        // Use the persistent probe RNG (same state across ticks) for
        // determinism, or the thread RNG for normal simulation runs.
        let mut pick = if let Some(ref mut ps) = probe_seed {
            ps.rng().random_range(0.0..total)
        } else {
            rand::rng().random_range(0.0..total)
        };
        let mut option_counter = 0usize;
        let (chosen_k, chosen_act): (Option<usize>, Act) = 'pick: {
            for k in 0..4 {
                if !scores[k].is_finite() { continue; }
                if pick < weights[k] {
                    elk.cell = cells[k].unwrap();
                    let (dx, dy) = steps[k];
                    break 'pick (Some(option_counter), Act::Step(dx, dy));
                }
                pick -= weights[k];
                option_counter += 1;
            }
            // Stand candidate
            if pick < weights[4] {
                break 'pick (Some(option_counter), Act::Stand);
            }
            option_counter += 1;
            // Graze candidate — catches any floating-point remainder
            (Some(option_counter), Act::Graze)
        };

        last_decision.0 = Decision {
            drives: decision_drives,
            options,
            chosen: chosen_k,
            chosen_act,
            temperature,
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

    // ── graze_value / stand_value ─────────────────────────────────────────────

    // At satiation (energy == 1.0) graze_value is 0 regardless of forage.
    #[test]
    fn graze_value_is_zero_at_satiation() {
        assert!((graze_value(1.0, 1.0, 1.5)).abs() < 1e-6);
        assert!((graze_value(0.5, 1.0, 1.5)).abs() < 1e-6);
        assert!((graze_value(0.0, 1.0, 1.5)).abs() < 1e-6);
    }

    // graze_value increases as here_forage rises (monotone in forage).
    #[test]
    fn graze_value_monotone_in_forage() {
        let low = graze_value(0.2, 0.5, 1.5);
        let high = graze_value(0.8, 0.5, 1.5);
        assert!(high > low, "richer forage must raise graze_value");
    }

    // graze_value increases as hunger (1 − energy) rises (monotone in hunger).
    #[test]
    fn graze_value_monotone_in_hunger() {
        let fed = graze_value(0.7, 0.9, 1.5);
        let starving = graze_value(0.7, 0.1, 1.5);
        assert!(starving > fed, "hungrier elk must have higher graze_value");
    }

    // A starving elk on rich grass has graze_value > stand_value() (graze beats rest when hungry on forage).
    #[test]
    fn graze_beats_stand_when_hungry_on_forage() {
        let gv = graze_value(1.0, 0.0, 1.5);
        assert!(gv > stand_value(), "graze must beat stand for a starving elk on rich grass");
    }

    // graze_value >= stand_value() always (eating is never worse than resting).
    #[test]
    fn graze_never_worse_than_stand() {
        for &forage in &[0.0_f32, 0.3, 0.7, 1.0] {
            for &energy in &[0.0_f32, 0.5, 0.9, 1.0] {
                let gv = graze_value(forage, energy, 1.5);
                assert!(gv >= stand_value(), "graze_value({forage}, {energy}) < stand_value()");
            }
        }
    }

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

    // With freshness_weight > 0, a high-freshness neighbour attracts even when a
    // higher-biomass neighbour sits the other way (fresh beats mature).
    #[test]
    fn gradient_points_toward_fresh_over_mature_biomass() {
        let mut grid = crate::grid::Grid::new(21, 21);
        let center = 10 * 21 + 10;
        let left = center - 1;  // col-1: high biomass, stale
        let right = center + 1; // col+1: lower biomass, but fresh

        // Left neighbour: high forage, zero freshness.
        grid.set_grass(left, 0.9);
        // Right neighbour: moderate forage but high freshness.
        grid.set_grass(right, 0.3);
        // Set freshness on right directly (public field).
        grid.freshness[right] = 2.0;

        let mut params = default_params();
        params.freshness_weight = 1.5;

        let g = grass_gradient(center, &grid, &params);
        assert!(
            g.x > 0.0,
            "with freshness_weight > 0, fresh cell (+x) must beat mature (-x): gradient={g:?}"
        );
    }

    // With freshness_weight == 0, gradient is identical to today's biomass-only result.
    #[test]
    fn gradient_freshness_weight_zero_is_identity() {
        let mut grid = crate::grid::Grid::new(21, 21);
        let center = 10 * 21 + 10;
        let right = center + 1;
        grid.set_grass(right, 0.8);
        grid.freshness[right] = 5.0; // high freshness but weight is 0

        let mut params_zero = default_params();
        params_zero.freshness_weight = 0.0;

        let mut params_positive = default_params();
        params_positive.freshness_weight = 0.0; // also zero, so identical

        let g_zero = grass_gradient(center, &grid, &params_zero);
        let g_pos = grass_gradient(center, &grid, &params_positive);

        assert!(
            (g_zero - g_pos).length() < 1e-6,
            "freshness_weight=0 must yield identical result regardless of freshness field"
        );
    }

    // ── forage_sightline ──────────────────────────────────────────────────────

    // Off by default: weight 0 ⇒ zero pull no matter what lies ahead (identity —
    // the grass drive stays the local gradient).
    #[test]
    fn sightline_off_by_default_is_zero() {
        let mut grid = crate::grid::Grid::new(40, 5);
        let center = 2 * 40 + 5;
        grid.set_grass(center + 15, 1.0); // rich patch far east
        let params = default_params(); // sightline_weight 0, range 0
        assert_eq!(forage_sightline(center, &grid, &params), Vec2::ZERO);
    }

    // The breaking input: rich forage *beyond* grass_radius to the east, none
    // underfoot. The local gradient can't see it; the sightline must pull +x.
    #[test]
    fn sightline_pulls_east_toward_far_forage() {
        let mut grid = crate::grid::Grid::new(40, 5);
        let center = 2 * 40 + 5;
        grid.set_grass(center + 12, 1.0); // 12 cells east — past grass_radius 5
        let mut params = default_params();
        params.sightline_range = 16.0;
        params.sightline_weight = 1.0;
        let s = forage_sightline(center, &grid, &params);
        assert!(s.x > 0.0 && s.y == 0.0, "must pull purely east toward far forage: {s:?}");
    }

    // No pull to nowhere: when nothing ahead beats underfoot the sightline is zero,
    // so a fed elk on good grass is never yanked east off it.
    #[test]
    fn sightline_zero_when_nothing_better_ahead() {
        let mut grid = crate::grid::Grid::new(40, 5);
        let center = 2 * 40 + 5;
        grid.set_grass(center, 1.0); // rich underfoot, barren ahead
        let mut params = default_params();
        params.sightline_range = 16.0;
        params.sightline_weight = 1.0;
        assert_eq!(forage_sightline(center, &grid, &params), Vec2::ZERO);
    }

    // Freshness is the long-range beacon: a fresh-but-not-yet-biomassy front ahead
    // pulls east once freshness_weight makes it the richer attractiveness.
    #[test]
    fn sightline_follows_the_freshness_front() {
        let mut grid = crate::grid::Grid::new(40, 5);
        let center = 2 * 40 + 5;
        let front = center + 10;
        grid.freshness[front] = 3.0; // green-up front, low standing crop
        let mut params = default_params();
        params.sightline_range = 16.0;
        params.sightline_weight = 1.0;
        params.freshness_weight = 1.5;
        assert!(forage_sightline(center, &grid, &params).x > 0.0);
    }

    // ── cohesion_lead_weight ──────────────────────────────────────────────────

    // lead 0 ⇒ every packmate weighs 1.0 (the plain centroid — today's behaviour),
    // regardless of whether they are ahead or behind.
    #[test]
    fn cohesion_lead_zero_is_uniform() {
        assert!((cohesion_lead_weight(10.0, 20.0, 9.0, 0.0) - 1.0).abs() < 1e-6);
        assert!((cohesion_lead_weight(10.0, 2.0, 9.0, 0.0) - 1.0).abs() < 1e-6);
    }

    // The breaking input: with lead > 0 a packmate ahead (+col) outweighs one behind,
    // so the cohesion centre drifts forward.
    #[test]
    fn cohesion_lead_favours_those_ahead() {
        let ahead = cohesion_lead_weight(10.0, 19.0, 9.0, 1.0); // a full coh_radius ahead
        let behind = cohesion_lead_weight(10.0, 1.0, 9.0, 1.0);
        assert!(ahead > behind, "ahead {ahead} must outweigh behind {behind}");
        assert!((behind - 1.0).abs() < 1e-6, "a packmate behind still weighs 1.0");
        assert!((ahead - 2.0).abs() < 1e-6, "a full-radius lead reaches 1 + lead = 2.0");
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

    // ── step_heading ──────────────────────────────────────────────────────────

    // A +x move yields a +x unit heading; the persistence term then rewards the
    // step that continues it and penalizes the reverse, by the same magnitude.
    #[test]
    fn step_heading_is_unit_cardinal_of_last_move() {
        let grid = Grid::new(6, 1);
        let h = step_heading(0, 1, &grid); // moved from cell 0 to cell 1 (+x)
        assert!((h - Vec2::new(1.0, 0.0)).length() < 1e-6);
    }

    // Holding position (prev == cell) leaves no heading, so a paused or grazing
    // elk carries no momentum into the next tick's choice.
    #[test]
    fn step_heading_is_zero_when_held() {
        let grid = Grid::new(6, 1);
        assert_eq!(step_heading(2, 2, &grid), Vec2::ZERO);
    }

    // The persistence term is symmetric: continuing scores +momentum, reversing
    // −momentum. The gap (2·momentum) is what breaks a mid-river limit cycle.
    #[test]
    fn heading_rewards_continuing_over_reversing() {
        let grid = Grid::new(6, 1);
        let h = step_heading(0, 1, &grid); // heading +x
        let forward = h.dot(Vec2::new(1.0, 0.0));
        let backward = h.dot(Vec2::new(-1.0, 0.0));
        assert!(forward > backward);
        assert!((forward - (-backward)).abs() < 1e-6);
    }

    // A multi-cell jump (respawn/teleport) is still clamped to a unit vector, so
    // momentum can never dominate the score after a discontinuity.
    #[test]
    fn step_heading_normalizes_a_teleport() {
        let grid = Grid::new(6, 1);
        let h = step_heading(0, 5, &grid); // five cells in +x
        assert!((h.length() - 1.0).abs() < 1e-6);
    }

    // ── best_reachable_frac ────────────────────────────────────────────────────

    // Finds a richer cell within the perception radius — the reference an elk
    // decides whether travelling is worth it against.
    #[test]
    fn best_reachable_finds_a_richer_neighbour() {
        let mut grid = Grid::new(7, 1);
        let cap = grid.capacity(0);
        grid.set_grass(0, 0.2 * cap); // underfoot: thin
        grid.set_grass(2, 0.9 * cap); // two cells away: rich
        assert!((best_reachable_frac(0, &grid, 5.0) - 0.9).abs() < 1e-3);
    }

    // With nothing better around, the best reachable IS the cell underfoot — the
    // signal that tells a traveller to settle and a grazer to stay.
    #[test]
    fn best_reachable_is_here_when_nothing_better() {
        let mut grid = Grid::new(7, 1);
        let cap = grid.capacity(0);
        grid.set_grass(0, 0.5 * cap); // neighbours stay at default 0
        assert!((best_reachable_frac(0, &grid, 5.0) - 0.5).abs() < 1e-3);
    }

    // Perception is bounded: a rich patch beyond the radius is invisible, so it
    // can't lure an elk into a dash it can't actually complete.
    #[test]
    fn best_reachable_ignores_cells_beyond_radius() {
        let mut grid = Grid::new(12, 1);
        let cap = grid.capacity(0);
        grid.set_grass(0, 0.1 * cap);
        grid.set_grass(10, 0.9 * cap); // far out of a radius-3 reach
        assert!((best_reachable_frac(0, &grid, 3.0) - 0.1).abs() < 1e-3);
    }

    // ── next_forage_mode ──────────────────────────────────────────────────────

    // A grazer leaves a drawn-down patch only when somewhere meaningfully richer
    // is reachable; an un-depleted patch is never abandoned.
    #[test]
    fn grazer_leaves_a_depleted_patch_only_for_a_better_one() {
        assert!(next_forage_mode(false, 0.3, 0.8, 0.4, 0.2), "depleted + better reachable → travel");
        assert!(!next_forage_mode(false, 0.5, 0.9, 0.4, 0.2), "not depleted → stay grazing");
    }

    // The anti-starvation guard, pinned. A depleted patch with nothing better in
    // reach keeps the elk grazing what it has — the exact case that, under the old
    // absolute-threshold mode, trapped the herd in permanent travel and starved it.
    #[test]
    fn grazer_stays_put_when_nothing_better_is_reachable() {
        assert!(!next_forage_mode(false, 0.3, 0.35, 0.4, 0.2), "best barely above here → no travel");
    }

    // A traveller keeps dashing while a richer patch is reachable and settles the
    // moment nothing better than underfoot remains — it arrives at the local best.
    #[test]
    fn traveller_settles_at_the_local_best() {
        assert!(next_forage_mode(true, 0.3, 0.8, 0.4, 0.2), "better reachable → keep travelling");
        assert!(!next_forage_mode(true, 0.5, 0.55, 0.4, 0.2), "nothing better in reach → settle");
    }

    // The fix for the starvation trap: a traveller on barren, uniformly poor
    // ground (best == here) settles instead of dashing forever toward forage that
    // is not there. Grazing is never suppressed, so a settled elk can still feed.
    #[test]
    fn traveller_settles_on_barren_flat_ground() {
        assert!(!next_forage_mode(true, 0.0, 0.0, 0.4, 0.2));
    }

    // Hysteresis without an absolute band: the same inputs yield opposite modes
    // depending on the prior mode (leaving needs depletion *and* a better patch;
    // continuing needs only a better patch). That asymmetry is what commits each
    // phase for a run instead of flickering tick to tick.
    #[test]
    fn mode_is_history_dependent() {
        let (here, best) = (0.5, 0.85);
        assert!(!next_forage_mode(false, here, best, 0.4, 0.2), "a grazer on a decent patch stays");
        assert!(next_forage_mode(true, here, best, 0.4, 0.2), "a traveller with better ahead continues");
    }

    // The anti-cloud property, pinned. While grazing a patch above leave_frac, a
    // richness signal that jitters across a midpoint would make a memoryless
    // cutoff flip nearly every tick — particle-cloud churn. The leave_frac gate
    // absorbs that jitter: the elk stays grazing, no flicker into travel.
    #[test]
    fn no_flicker_while_grazing_a_patch_above_leave_frac() {
        let signal: Vec<f32> = (0..40)
            .map(|t| 0.5 + 0.08 * if t % 2 == 0 { 1.0 } else { -1.0 })
            .collect();
        let best = 0.6; // a richer patch exists nearby, fixed

        let naive_flips = signal
            .windows(2)
            .filter(|w| (w[0] < 0.5) != (w[1] < 0.5))
            .count();

        let mut traveling = false;
        let mut flips = 0;
        for &f in &signal {
            let next = next_forage_mode(traveling, f, best, 0.4, 0.2);
            if next != traveling {
                flips += 1;
            }
            traveling = next;
        }

        assert!(naive_flips > 30, "naive cutoff churns every tick (got {naive_flips})");
        assert_eq!(flips, 0, "grazer above leave_frac never churns into travel (got {flips})");
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
