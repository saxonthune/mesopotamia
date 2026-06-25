//! The herding movement model — a plan-then-steer successor to the per-tick drive
//! accumulator (`movement::herd_move`, doc03.01.09).
//!
//! Each elk runs a small state machine (graze / travel / cross). Each state drives
//! **steering behaviours** — arrive, separation, cohesion — that produce a desired
//! heading, which is then quantised to one of the eight grid steps and committed as a
//! whole-cell move. Movement stays grid-locked (an elk only ever sits on a real
//! cell), and the renderer slides the sprite `prev_cell → cell` so the motion is
//! smooth even though the logic is discrete. Committing to a whole step — rather than
//! re-rolling a heading every tick — is what removes the buzzing. Leadership is not a
//! state: it is the confidence weight that blends an elk's own goal against cohesion
//! with the herd, so the column emerges with no designated leader and direction stays
//! a property of the forage field, not a hard-coded compass.
//!
//! The forage and crossing *scorers* are reused unchanged from `movement` — this
//! module replaces only how a destination becomes motion. Feeding is decoupled: the
//! grazing system reads each elk's `Herding` state directly and feeds whenever food
//! sits underfoot and the elk is not mid-crossing, so the movement model can never
//! starve a herd standing on food.

use bevy::prelude::*;

use crate::grid::Grid;

use super::components::{Elk, ElkParams};
use super::ledger::EnergyFlows;
use super::movement::{cross_desire, forage_across, forage_sightline, grass_gradient, swim_cost};

/// A water level at or above this makes a cell count as water for crossing.
const WATER_EPS: f32 = 0.01;

/// One elk's committed movement state: which behaviour it runs and the goal that
/// behaviour steers toward. Overwritten in place each tick (never inserted or
/// removed) so the archetype never churns — same discipline as `LastDecision`.
#[derive(Component)]
pub struct Herding {
    pub state: HerdState,
    pub goal: Goal,
    /// Ticks the current state has been held. Drives the give-up timeout so a stale
    /// travel goal can never strand an elk forever.
    pub dwell: u32,
}

impl Default for Herding {
    fn default() -> Self {
        Herding { state: HerdState::Graze, goal: Goal::None, dwell: 0 }
    }
}

/// The three live states. `Flee` (predator response) is left for a later milestone.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HerdState {
    /// Area-restricted search: hold roughly in place and feed.
    Graze,
    /// Directed relocation: Arrive at a committed destination.
    Travel,
    /// Committed river traversal: lock onto the far bank and pay the swim cost.
    Cross,
}

/// The committed destination — the thing deliberately *not* re-rolled each tick.
/// Following the herd is not here: it is emergent per-tick cohesion steering scaled
/// by `1 - confidence`, so there is no stored follow-target to dangle.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Goal {
    /// No destination (Graze): the steer is a gentle local pull.
    None,
    /// A grid cell to Arrive at — a richer patch picked by the forage scorers.
    Patch(usize),
    /// The dry cell on the far side of a committed crossing.
    FarBank(usize),
}

/// Every tunable that shapes the herding steer. Speeds are the **slide rate** in grid
/// cells per tick — the fraction of a cell-step animated per tick — so a `max_speed`
/// below 1 makes a step take several ticks and the herd glide forward slower than a
/// cell per tick.
#[derive(Resource, Clone)]
pub struct HerdParams {
    /// Travel slide rate (cells/tick): how fast a committed step animates. Small ⇒ a
    /// slow, graceful roll; a step at 0.5 takes two ticks.
    pub max_speed: f32,
    /// Graze slide rate — far below `max_speed`, so the occasional spacing step a
    /// feeding elk takes drifts in slowly.
    pub graze_speed: f32,
    /// Begin decelerating inside this distance of the goal (cells).
    pub slow_radius: f32,
    /// Within this distance the elk has arrived and settles (cells).
    pub arrive_radius: f32,
    /// Weight on the separation push — kept low so it only nudges a crowded elk one
    /// cell aside, never a continuous repulsion field that stalls the herd.
    pub separation: f32,
    /// Weight on cohesion — the "follow the herd" pull, amplified as confidence falls.
    pub cohesion: f32,
    pub sep_radius: f32,
    pub coh_radius: f32,
    /// Forage-signal magnitude at which an elk fully trusts its own goal (confidence 0.5).
    pub confidence_ref: f32,
    /// How far an elk scans for a travel destination (cells).
    pub scan_radius: f32,
    /// Graze→Travel: leave a patch once its grass falls below this fraction of capacity.
    pub leave_frac: f32,
    /// A scanned patch must beat underfoot by this attractiveness to be worth travelling to.
    pub travel_margin: f32,
    /// Abandon a travel goal that has not been reached within this many ticks.
    pub goal_timeout: u32,
    /// Minimum desired-heading magnitude that commits a step. A pull below this leaves
    /// the elk settled — the hysteresis dead-band (Millington 13.2.3) that stops an
    /// elk jittering between cells on a negligible nudge.
    pub deadband: f32,
}

impl Default for HerdParams {
    fn default() -> Self {
        Self {
            max_speed: 0.5,
            graze_speed: 0.1,
            slow_radius: 4.0,
            arrive_radius: 0.6,
            separation: 0.5,
            cohesion: 0.5,
            sep_radius: 1.5,
            coh_radius: 8.0,
            confidence_ref: 0.5,
            scan_radius: 8.0,
            leave_frac: 0.4,
            travel_margin: 0.05,
            goal_timeout: 200,
            deadband: 0.15,
        }
    }
}

// ── Pure kernels ────────────────────────────────────────────────────────────────

/// Desired velocity toward `target` with an arrival slowdown: full `max_speed`
/// outside `slow_r`, ramping linearly to zero at the target, and exactly zero once
/// within `arrive_r`. This is the anti-overshoot primitive — a plain seek orbits and
/// oscillates around a fixed target; arrive settles onto it.
pub fn arrive(pos: Vec2, target: Vec2, slow_r: f32, arrive_r: f32, max_speed: f32) -> Vec2 {
    let to = target - pos;
    let dist = to.length();
    if dist < arrive_r || dist < 1e-6 {
        return Vec2::ZERO;
    }
    let speed = if dist > slow_r { max_speed } else { max_speed * dist / slow_r.max(1e-6) };
    to / dist * speed
}

/// Repulsion from neighbours within `radius`, falling off with inverse square so a
/// near crowd pushes harder than a far one. `others` are neighbour positions; the
/// elk's own position contributes nothing (zero offset is skipped).
pub fn separation(pos: Vec2, others: &[Vec2], radius: f32) -> Vec2 {
    let r2 = radius * radius;
    let mut acc = Vec2::ZERO;
    for &o in others {
        let off = pos - o;
        let d2 = off.length_squared();
        if d2 > 1e-6 && d2 < r2 {
            acc += off / d2;
        }
    }
    acc
}

/// An elk's confidence in its own goal, in [0, 1]: `signal / (signal + reference)`.
/// Zero with no forage signal, half at `reference`, approaching 1 for a steep
/// gradient. The blend weight that makes a well-informed elk lead and a poorly
/// informed one follow — the leaderless-leadership knob (Couzin et al. 2005).
pub fn confidence(signal: f32, reference: f32) -> f32 {
    let s = signal.max(0.0);
    s / (s + reference.max(1e-6))
}

/// Quantise a desired heading to one of the eight grid steps `(dx, dy)`, or `None` to
/// stay put when the heading is shorter than `deadband`. The angle is binned into 45°
/// octants centred on each compass direction, so the chosen step is the neighbour cell
/// nearest the desired heading. This is what keeps movement grid-locked: a continuous
/// steer becomes one whole-cell move, never a sub-cell drift.
pub fn quantize_step(desired: Vec2, deadband: f32) -> Option<(isize, isize)> {
    if desired.length() < deadband {
        return None;
    }
    let oct = (desired.y.atan2(desired.x) / std::f32::consts::FRAC_PI_4).round() as i32;
    let step = match oct.rem_euclid(8) {
        0 => (1, 0),
        1 => (1, 1),
        2 => (0, 1),
        3 => (-1, 1),
        4 => (-1, 0),
        5 => (-1, -1),
        6 => (0, -1),
        7 => (1, -1),
        _ => unreachable!(),
    };
    Some(step)
}

/// Whether to leave the patch underfoot for travel: the grass here is drawn below
/// `leave_frac` of capacity *and* a reachable patch is at least `margin` richer.
/// Requiring both is the hysteresis that keeps a herd grazing where nothing better
/// exists instead of dashing off thin ground.
pub fn should_leave_patch(here_frac: f32, best_frac: f32, leave_frac: f32, margin: f32) -> bool {
    here_frac < leave_frac && best_frac > here_frac + margin
}

// ── Grid position helpers ────────────────────────────────────────────────────────

/// The centre point (in cell coordinates) of a grid cell.
fn cell_center(cell: usize, grid: &Grid) -> Vec2 {
    let (c, r) = grid.col_row(cell);
    Vec2::new(c as f32 + 0.5, r as f32 + 0.5)
}

/// Forage attractiveness of a cell — standing crop plus the green-up front, the same
/// measure the gradient and sightline climb.
fn attract(cell: usize, grid: &Grid, ep: &ElkParams) -> f32 {
    grid.forage(cell) + ep.freshness_weight * grid.freshness(cell)
}

/// Best food fullness (`food_frac`) over cells within `radius` of `here`, the cell
/// itself included — the reachability reference the patch-leaving gate steers by.
/// Reads the food boundary, so depletion of either grass or shrubs registers and a
/// herd only leaves a patch when somewhere genuinely fuller is in reach.
fn best_reachable_food_frac(here: usize, grid: &Grid, radius: f32) -> f32 {
    let r = radius.ceil() as isize;
    let mut best = grid.food_frac(here);
    for dy in -r..=r {
        for dx in -r..=r {
            if dx == 0 && dy == 0 {
                continue;
            }
            if (Vec2::new(dx as f32, dy as f32)).length() > radius {
                continue;
            }
            if let Some(n) = grid.step(here, dx, dy) {
                best = best.max(grid.food_frac(n));
            }
        }
    }
    best
}

/// Pick the richest reachable cell within `radius` of `here`. Returns the argmax cell
/// and its attractiveness; the caller decides whether it beats underfoot by the
/// travel margin. The scan is unbiased in direction — forward motion emerges from
/// where the forage (and the green-up front) actually is, never a hard-coded axis.
fn richest_within(here: usize, grid: &Grid, ep: &ElkParams, radius: f32) -> (usize, f32) {
    let r = radius.ceil() as isize;
    let mut best_cell = here;
    let mut best_val = attract(here, grid, ep);
    for dy in -r..=r {
        for dx in -r..=r {
            if dx == 0 && dy == 0 {
                continue;
            }
            if (Vec2::new(dx as f32, dy as f32)).length() > radius {
                continue;
            }
            if let Some(n) = grid.step(here, dx, dy) {
                let v = attract(n, grid, ep);
                if v > best_val {
                    best_val = v;
                    best_cell = n;
                }
            }
        }
    }
    (best_cell, best_val)
}

// ── The system ───────────────────────────────────────────────────────────────────

/// The herding movement system. Two phases: phase 1 snapshots every elk's cell centre
/// for neighbour reads, phase 2 runs the state machine, blends the steering into a
/// desired heading, quantises that to a grid step, and commits a whole-cell move
/// (advancing the render slide). It also applies the swim energy cost.
pub(super) fn herd_step(
    grid: Res<Grid>,
    hp: Res<HerdParams>,
    ep: Res<ElkParams>,
    mut elk_q: Query<(&mut Elk, &mut Herding)>,
    mut flows: ResMut<EnergyFlows>,
) {
    // Phase 1: snapshot (cell centre, slot) for every elk in query order — movement is
    // grid-locked, so the neighbour geometry is read off cell centres.
    let snapshot: Vec<(Vec2, u8)> =
        elk_q.iter().map(|(elk, _)| (cell_center(elk.cell, &grid), elk.slot)).collect();

    for (i, (mut elk, mut herd)) in elk_q.iter_mut().enumerate() {
        let (pos, slot) = snapshot[i];

        // Neighbour drives from the snapshot: separation from all, cohesion to the
        // same-slot centroid. Both in cell-coordinate distances.
        let mut others = Vec::new();
        let mut coh_sum = Vec2::ZERO;
        let mut coh_n = 0.0_f32;
        for (j, &(opos, oslot)) in snapshot.iter().enumerate() {
            if j == i {
                continue;
            }
            others.push(opos);
            if oslot == slot && (opos - pos).length() <= hp.coh_radius {
                coh_sum += opos;
                coh_n += 1.0;
            }
        }
        let sep = separation(pos, &others, hp.sep_radius) * hp.separation;
        let coh_dir = if coh_n > 0.0 { (coh_sum / coh_n - pos).normalize_or_zero() } else { Vec2::ZERO };

        // Own forage signal: the local gradient plus the long-range sightline. Its
        // magnitude is the confidence that decides leading vs following.
        let here = elk.cell;
        let signal = grass_gradient(here, &grid, &ep) + forage_sightline(here, &grid, &ep);
        let conf = confidence(signal.length(), hp.confidence_ref);

        // Fullness underfoot through the food boundary (grass + shrubs), so a herd
        // on the dry steppe reads its shrubs the way a riparian herd reads its grass.
        let here_frac = grid.food_frac(here);

        herd.dwell = herd.dwell.saturating_add(1);

        // Run the state machine: produce a desired heading and (maybe) transition.
        // The desired vector's *direction* picks the grid step and its *magnitude*
        // gates the dead-band; the state shapes movement only — feeding is decoupled
        // below so grazing is never suppressed (a travelling elk over food still eats).
        let desired;
        match herd.state {
            HerdState::Graze => {
                // Hold and feed: separation relieves crowding, a faint cohesion drift
                // rides under the dead-band — so a settled, well-spaced grazer takes no
                // step, and only a crowded one nudges a cell aside.
                desired = sep + coh_dir * hp.graze_speed;
                let best_frac = best_reachable_food_frac(here, &grid, ep.grass_radius);
                if should_leave_patch(here_frac, best_frac, hp.leave_frac, hp.travel_margin) {
                    let (cell, val) = richest_within(here, &grid, &ep, hp.scan_radius);
                    if cell != here && val > attract(here, &grid, &ep) + hp.travel_margin {
                        herd.state = HerdState::Travel;
                        herd.goal = Goal::Patch(cell);
                        herd.dwell = 0;
                    }
                }
            }
            HerdState::Travel => {
                let target_cell = match herd.goal {
                    Goal::Patch(c) => c,
                    _ => here,
                };
                let target = cell_center(target_cell, &grid);
                let arrive_v = arrive(pos, target, hp.slow_radius, hp.arrive_radius, hp.max_speed);
                // Blend: trust own arrive by confidence, fall back to the herd as it drops.
                let follow = coh_dir * (hp.max_speed * hp.cohesion * (1.0 - conf));
                desired = arrive_v * conf + follow + sep;

                // Crossing check: if the cell one step ahead is water, weigh a ford.
                if let Some((far, cross)) = crossing_ahead(pos, desired, here, &grid, &ep) {
                    if cross {
                        herd.state = HerdState::Cross;
                        herd.goal = Goal::FarBank(far);
                        herd.dwell = 0;
                    }
                }

                // Settle on arrival, on a stale goal, or when nothing better remains.
                let arrived = (target - pos).length() < hp.arrive_radius;
                if arrived || herd.dwell > hp.goal_timeout || target_cell == here {
                    herd.state = HerdState::Graze;
                    herd.goal = Goal::None;
                    herd.dwell = 0;
                }
            }
            HerdState::Cross => {
                let target_cell = match herd.goal {
                    Goal::FarBank(c) => c,
                    _ => here,
                };
                let target = cell_center(target_cell, &grid);
                // Committed: arrive hard at the far bank, ignore the herd's backward pull.
                desired = arrive(pos, target, hp.slow_radius, hp.arrive_radius, hp.max_speed) + sep;
                // Once on dry land, the crossing is done.
                if grid.water(here) < WATER_EPS && (target - pos).length() < hp.slow_radius {
                    herd.state = HerdState::Graze;
                    herd.goal = Goal::None;
                    herd.dwell = 0;
                }
            }
        }

        elk.traveling = herd.state != HerdState::Graze;

        // Grid-locked motion: commit a whole-cell step only when settled, then animate
        // the slide. Committing the entire step (rather than a sub-cell drift) is the
        // anti-buzz hysteresis — the elk cannot reverse mid-step — and the renderer
        // turns the discrete move into smooth motion by lerping prev_cell → cell.
        let rate = match herd.state {
            HerdState::Graze => hp.graze_speed,
            _ => hp.max_speed,
        };
        if elk.move_t >= 1.0 {
            match quantize_step(desired, hp.deadband).and_then(|(dx, dy)| grid.step(here, dx, dy)) {
                Some(n) => {
                    elk.prev_cell = here;
                    elk.cell = n;
                    elk.move_t = 0.0;
                    elk.move_rate = rate;
                }
                // No step worth taking: settle flat on the cell (no residual slide).
                None => elk.move_rate = 0.0,
            }
        }
        if elk.move_t < 1.0 {
            elk.move_t = (elk.move_t + elk.move_rate).min(1.0);
        }

        // Swim energy cost: crossing deep non-ford water tires the elk.
        let water = grid.water(elk.cell);
        if water > 0.0 && !grid.is_ford(elk.cell) {
            let before = elk.energy;
            elk.energy = (elk.energy - ep.swim_drain * water).max(0.0);
            flows.swim += before - elk.energy;
        }
    }
}

/// If the cell one step ahead along `heading` is water, look across for the far bank
/// and decide whether the ford is worth it. Returns `(far_bank_cell, worth_it)`, or
/// `None` when no water lies immediately ahead. Reuses the crossing scorers verbatim.
fn crossing_ahead(
    pos: Vec2,
    heading: Vec2,
    here: usize,
    grid: &Grid,
    ep: &ElkParams,
) -> Option<(usize, bool)> {
    if heading.length() < 1e-3 {
        return None;
    }
    let (dx, dy) = if heading.x.abs() >= heading.y.abs() {
        (if heading.x >= 0.0 { 1 } else { -1 }, 0)
    } else {
        (0, if heading.y >= 0.0 { 1 } else { -1 })
    };
    let ahead = grid.step(here, dx, dy)?;
    if grid.water(ahead) < WATER_EPS {
        return None;
    }
    let peek = ep.cross_peek.max(1.0) as usize;
    let (far, eff) = forage_across(grid, here, dx, dy, ep.ford_discount, peek)?;
    let _ = pos;
    let here_attract = attract(here, grid, ep);
    let across = attract(far, grid, ep);
    let cost = swim_cost(eff, ep.swim_reluctance);
    // `ahead` is a water cell (≈ 0 forage); the dry option is staying put.
    Some((far, cross_desire(here_attract, here_attract, across, cost) > 0.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── arrive ──────────────────────────────────────────────────────────────────

    // Outside the slow radius the elk moves at full speed toward the target.
    #[test]
    fn arrive_full_speed_when_far() {
        let v = arrive(Vec2::ZERO, Vec2::new(10.0, 0.0), 4.0, 0.6, 0.5);
        assert!((v.length() - 0.5).abs() < 1e-6, "should be max speed, got {}", v.length());
        assert!(v.x > 0.0 && v.y.abs() < 1e-6, "should aim +x: {v:?}");
    }

    // Within the arrive radius the desired velocity is zero — it settles, not orbits.
    #[test]
    fn arrive_settles_at_target() {
        let v = arrive(Vec2::ZERO, Vec2::new(0.3, 0.0), 4.0, 0.6, 0.5);
        assert_eq!(v, Vec2::ZERO);
    }

    // Inside the slow radius speed scales down with distance (metamorphic: nearer ⇒ slower).
    #[test]
    fn arrive_slows_approaching() {
        let far = arrive(Vec2::ZERO, Vec2::new(3.0, 0.0), 4.0, 0.6, 0.5).length();
        let near = arrive(Vec2::ZERO, Vec2::new(1.5, 0.0), 4.0, 0.6, 0.5).length();
        assert!(near < far, "closer must be slower: {near} !< {far}");
    }

    // ── confidence ────────────────────────────────────────────────────────────────

    // Zero signal ⇒ zero confidence; at the reference it is exactly one half.
    #[test]
    fn confidence_endpoints() {
        assert_eq!(confidence(0.0, 0.5), 0.0);
        assert!((confidence(0.5, 0.5) - 0.5).abs() < 1e-6);
    }

    // Monotone increasing in signal — a steeper gradient never lowers confidence.
    #[test]
    fn confidence_monotone_in_signal() {
        let lo = confidence(0.2, 0.5);
        let hi = confidence(2.0, 0.5);
        assert!(hi > lo, "stronger signal must raise confidence: {hi} !> {lo}");
        assert!(hi < 1.0, "confidence stays below 1");
    }

    // ── quantize_step ─────────────────────────────────────────────────────────────

    // A heading below the dead-band commits no step — the anti-jitter hysteresis.
    #[test]
    fn quantize_stays_put_under_deadband() {
        assert_eq!(quantize_step(Vec2::new(0.05, 0.0), 0.15), None);
        assert_eq!(quantize_step(Vec2::ZERO, 0.15), None);
    }

    // Each compass heading snaps to its own grid step (the eight octants).
    #[test]
    fn quantize_picks_the_nearest_of_eight() {
        assert_eq!(quantize_step(Vec2::new(1.0, 0.0), 0.1), Some((1, 0)));
        assert_eq!(quantize_step(Vec2::new(0.0, 1.0), 0.1), Some((0, 1)));
        assert_eq!(quantize_step(Vec2::new(-1.0, 0.0), 0.1), Some((-1, 0)));
        assert_eq!(quantize_step(Vec2::new(0.0, -1.0), 0.1), Some((0, -1)));
        // A near-45° heading takes the diagonal step.
        assert_eq!(quantize_step(Vec2::new(1.0, 1.0), 0.1), Some((1, 1)));
        assert_eq!(quantize_step(Vec2::new(-1.0, -0.9), 0.1), Some((-1, -1)));
    }

    // A heading just off an axis still snaps to that axis (within its 45° octant).
    #[test]
    fn quantize_snaps_within_octant() {
        // 20° above +x is well inside the east octant (±22.5°).
        let v = Vec2::new(1.0, 0.36);
        assert_eq!(quantize_step(v, 0.1), Some((1, 0)));
    }

    // ── separation ────────────────────────────────────────────────────────────────

    // A neighbour to the right pushes the elk left; a far neighbour is ignored.
    #[test]
    fn separation_pushes_away_and_ignores_far() {
        let near = separation(Vec2::ZERO, &[Vec2::new(1.0, 0.0)], 2.5);
        assert!(near.x < 0.0, "should push away from the right neighbour: {near:?}");
        let far = separation(Vec2::ZERO, &[Vec2::new(100.0, 0.0)], 2.5);
        assert_eq!(far, Vec2::ZERO, "a neighbour beyond the radius exerts no push");
    }

    // Closer neighbours push harder than distant ones (inverse-square, metamorphic).
    #[test]
    fn separation_stronger_when_closer() {
        let close = separation(Vec2::ZERO, &[Vec2::new(0.5, 0.0)], 2.5).length();
        let further = separation(Vec2::ZERO, &[Vec2::new(2.0, 0.0)], 2.5).length();
        assert!(close > further, "closer crowd must push harder: {close} !> {further}");
    }

    // ── food boundary ──────────────────────────────────────────────────────────────

    // On dry steppe (no grass capacity) the food fraction reads the shrubs, so a
    // depleting shrub clump registers as a draining patch — the grass-only metric
    // could not see this, which is what pinned the herd in place.
    #[test]
    fn food_frac_reads_shrubs_where_grass_cannot_grow() {
        let mut grid = Grid::new(5, 5);
        // A dry cell: zero grass capacity (no water proximity), but it carries shrubs.
        grid.set_water_prox(12, 0.0);
        grid.set_shrub_cap(12, 1.0);
        grid.set_shrubs(12, 0.8);
        assert!((grid.food_frac(12) - 0.8).abs() < 1e-5, "food_frac must reflect shrub fullness");
        // Eating the shrubs down drops the fraction — the leave-gate can now see it.
        grid.eat_shrubs(12, 0.6);
        assert!(grid.food_frac(12) < 0.3, "depleting shrubs must lower food_frac");
    }

    // A barren cell (no grass, no shrub capacity) reads empty, not NaN.
    #[test]
    fn food_frac_is_zero_on_carryless_ground() {
        let grid = Grid::new(5, 5);
        assert_eq!(grid.food_frac(12), 0.0);
    }

    // ── should_leave_patch ─────────────────────────────────────────────────────────

    // Leaves only when the patch is drawn down AND somewhere better is reachable.
    #[test]
    fn leaves_only_on_depletion_and_better_reachable() {
        // Depleted here (0.2 < 0.4) and a richer patch (0.7) reachable → leave.
        assert!(should_leave_patch(0.2, 0.7, 0.4, 0.05));
        // Depleted but nothing better reachable → stay and graze.
        assert!(!should_leave_patch(0.2, 0.22, 0.4, 0.05));
        // Rich underfoot → stay even if something richer exists.
        assert!(!should_leave_patch(0.8, 0.95, 0.4, 0.05));
    }
}
