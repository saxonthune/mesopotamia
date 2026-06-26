//! Herding movement model (doc03.01.09): plan-then-steer over grid-locked whole-cell steps.
//! Feeding is decoupled — the grazer reads `Herding` state directly and is never blocked by movement.

use bevy::prelude::*;

use crate::grid::Grid;

use super::components::{Elk, ElkParams};
use super::ledger::EnergyFlows;
use super::movement::{cross_desire, forage_across, forage_sightline, grass_gradient, swim_cost};

const WATER_EPS: f32 = 0.01;

// Separation owns below this range, cohesion above — the gap is the loose clump.
const GRAZE_COH_DEAD: f32 = 0.8;

// Full cohesion pull beyond this, ramping to zero at GRAZE_COH_DEAD; kept small or the herd settles into a hollow ring.
const GRAZE_COH_SLOW: f32 = 2.5;

// Stacked elk have d²=0 (skips inverse-square); each gets escape direction i·GOLDEN_ANGLE to fan out maximally.
const GOLDEN_ANGLE: f32 = 2.399_963_2;

/// Elk movement state — overwritten each tick, never inserted/removed (no archetype churn).
#[derive(Component)]
pub struct Herding {
    pub state: HerdState,
    pub goal: Goal,
    /// Ticks held in current state; drives the give-up timeout for stale travel goals.
    pub dwell: u32,
    /// Ticks left chewing — while nonzero the Graze steer is pinned to zero, physically halting the herd.
    pub chew: u32,
}

impl Default for Herding {
    fn default() -> Self {
        Herding { state: HerdState::Graze, goal: Goal::None, dwell: 0, chew: 0 }
    }
}

impl Herding {
    pub fn is_traveling(&self) -> bool {
        self.state != HerdState::Graze
    }

    /// Metabolism reads this contract; new states set their own feeding policy here.
    pub fn permits_feeding(&self) -> bool {
        self.state != HerdState::Cross
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HerdState {
    /// Area-restricted search: hold roughly in place and feed.
    Graze,
    /// Directed relocation: Arrive at a committed destination.
    Travel,
    /// Committed river traversal: lock onto the far bank and pay the swim cost.
    Cross,
}

/// Committed destination — not re-rolled each tick. Herd following is emergent per-tick cohesion, not a stored target.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Goal {
    /// No destination (Graze): the steer is a gentle local pull.
    None,
    Patch(usize),
    FarBank(usize),
}

/// Every tunable for herding. Speeds are slide rates (cells/tick); below 1 makes a step take multiple ticks.
#[derive(Resource, Clone)]
pub struct HerdParams {
    pub max_speed: f32,
    pub graze_speed: f32,
    pub slow_radius: f32,
    pub arrive_radius: f32,
    pub separation: f32,
    pub cohesion: f32,
    /// Velocity matching (Millington §3.3.6); kept below cohesion (separation > cohesion > alignment ordering).
    pub alignment: f32,
    /// Anisotropic perception (Couzin 2002): biases cohesion toward kin ahead, polarising a moving herd into a column.
    pub forward_bias: f32,
    pub sep_radius: f32,
    pub coh_radius: f32,
    /// Forage-signal magnitude at which confidence is exactly 0.5.
    pub confidence_ref: f32,
    pub scan_radius: f32,
    pub leave_frac: f32,
    /// Rest floor: ticks an elk must graze before relocating — stops perpetual travel up a gradient.
    pub graze_min_dwell: u32,
    pub travel_margin: f32,
    /// Gain above underfoot that triggers migration even from a full patch — the green-up crest a fed herd chases.
    pub pursue_margin: f32,
    pub goal_timeout: u32,
    /// Millington 13.2.3 dead-band: pulls weaker than this leave the elk settled (anti-jitter).
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
            alignment: 0.3,
            forward_bias: 0.5,
            sep_radius: 1.5,
            coh_radius: 8.0,
            confidence_ref: 0.5,
            scan_radius: 8.0,
            leave_frac: 0.4,
            graze_min_dwell: 30,
            travel_margin: 0.05,
            pursue_margin: 0.15,
            goal_timeout: 200,
            deadband: 0.15,
        }
    }
}

/// plain seek orbits; arrive settles by ramping speed to zero within slow_r.
pub fn arrive(pos: Vec2, target: Vec2, slow_r: f32, arrive_r: f32, max_speed: f32) -> Vec2 {
    let to = target - pos;
    let dist = to.length();
    if dist < arrive_r || dist < 1e-6 {
        return Vec2::ZERO;
    }
    let speed = if dist > slow_r { max_speed } else { max_speed * dist / slow_r.max(1e-6) };
    to / dist * speed
}

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

/// Couzin 2002 anisotropic centroid: `fwd_bias` weights kin ahead of `heading` more, polarising a moving herd into a column.
/// Falls back to nearest kin when none are in range, so stragglers are always reeled in.
pub fn cohesion_target(pos: Vec2, kin: &[Vec2], heading: Vec2, coh_radius: f32, fwd_bias: f32) -> Option<Vec2> {
    let r2 = coh_radius * coh_radius;
    let h = heading.normalize_or_zero();
    let mut wsum = Vec2::ZERO;
    let mut w = 0.0_f32;
    let mut nearest: Option<(f32, Vec2)> = None;
    for &k in kin {
        let off = k - pos;
        let d2 = off.length_squared();
        if d2 <= r2 {
            let cos = if d2 > 1e-6 && h != Vec2::ZERO { off.normalize().dot(h) } else { 0.0 };
            let weight = (1.0 + fwd_bias * cos).max(0.0);
            wsum += k * weight;
            w += weight;
        }
        if nearest.is_none_or(|(bd, _)| d2 < bd) {
            nearest = Some((d2, k));
        }
    }
    if w > 0.0 {
        Some(wsum / w)
    } else {
        nearest.map(|(_, p)| p)
    }
}

/// Couzin et al. 2005 leaderless leadership: sigmoid of forage signal — high gradient leads, flat field follows.
pub fn confidence(signal: f32, reference: f32) -> f32 {
    let s = signal.max(0.0);
    s / (s + reference.max(1e-6))
}

/// Keeps movement grid-locked: continuous steer → nearest of 8 compass steps, or None under `deadband`.
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

/// Both gates required: depleted *and* somewhere better — hysteresis that holds a herd on thin ground when nothing richer is reachable.
pub fn should_leave_patch(here_frac: f32, best_frac: f32, leave_frac: f32, margin: f32) -> bool {
    here_frac < leave_frac && best_frac > here_frac + margin
}

/// dwell floor: without it a herd on a gradient leaves for a marginally richer cell every tick and travels forever.
pub fn should_leave_graze(
    dwell: u32,
    min_dwell: u32,
    here_frac: f32,
    best_frac: f32,
    leave_frac: f32,
    margin: f32,
) -> bool {
    dwell >= min_dwell && should_leave_patch(here_frac, best_frac, leave_frac, margin)
}

fn cell_center(cell: usize, grid: &Grid) -> Vec2 {
    let (c, r) = grid.col_row(cell);
    Vec2::new(c as f32 + 0.5, r as f32 + 0.5)
}

fn attract(cell: usize, grid: &Grid, ep: &ElkParams) -> f32 {
    grid.forage(cell) + ep.freshness_weight * grid.freshness(cell)
}

// Reads food_frac (grass+shrubs) so a dry-steppe shrub depletion registers — the grass-only metric could never see it.
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

// Scan is direction-unbiased; forward motion emerges from where the forage actually is, not a hard-coded axis.
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

pub(super) fn herd_step(
    grid: Res<Grid>,
    hp: Res<HerdParams>,
    ep: Res<ElkParams>,
    mut elk_q: Query<(&mut Elk, &mut Herding)>,
    mut flows: ResMut<EnergyFlows>,
) {
    // heading is nonzero only while moving — a settled elk contributes nothing to alignment.
    let snapshot: Vec<(Vec2, Vec2, u8)> = elk_q
        .iter()
        .map(|(elk, _)| {
            let pos = cell_center(elk.cell, &grid);
            let vel = if elk.move_rate > 0.0 {
                pos - cell_center(elk.prev_cell, &grid)
            } else {
                Vec2::ZERO
            };
            (pos, vel, elk.slot)
        })
        .collect();

    for (i, (mut elk, mut herd)) in elk_q.iter_mut().enumerate() {
        let (pos, vel, slot) = snapshot[i];

        let mut others = Vec::new();
        let mut kin = Vec::new();
        let mut align_sum = Vec2::ZERO;
        let mut align_n = 0.0_f32;
        let mut coincident = 0u32;
        for (j, &(opos, ovel, oslot)) in snapshot.iter().enumerate() {
            if j == i {
                continue;
            }
            others.push(opos);
            if opos == pos {
                coincident += 1;
            }
            if oslot == slot {
                kin.push(opos);
                if (opos - pos).length() <= hp.coh_radius {
                    align_sum += ovel.normalize_or_zero();
                    align_n += 1.0;
                }
            }
        }
        // separation can't split a zero-distance stack (d²=0 guard); GOLDEN_ANGLE gives each a distinct escape direction.
        let stack_kick = if coincident > 0 {
            Vec2::from_angle(i as f32 * GOLDEN_ANGLE) * hp.separation * coincident as f32
        } else {
            Vec2::ZERO
        };
        let sep = separation(pos, &others, hp.sep_radius) * hp.separation + stack_kick;
        let coh_center = cohesion_target(pos, &kin, vel, hp.coh_radius, hp.forward_bias).unwrap_or(pos);
        let coh_dir = (coh_center - pos).normalize_or_zero();
        // averaging unit headings: magnitude ≈ 1 when coherent, ≈ 0 when scattered.
        let align = if align_n > 0.0 { (align_sum / align_n) * hp.alignment } else { Vec2::ZERO };

        let here = elk.cell;
        let signal = grass_gradient(here, &grid, &ep) + forage_sightline(here, &grid, &ep);
        let conf = confidence(signal.length(), hp.confidence_ref);

        let here_frac = grid.food_frac(here);

        herd.dwell = herd.dwell.saturating_add(1);

        // Evaluated independent of move state so a grazing herd beside a river considers the ford on forage merits alone.
        if herd.state != HerdState::Cross {
            if let Some(far) = best_crossing(here, &grid, &ep) {
                herd.state = HerdState::Cross;
                herd.goal = Goal::FarBank(far);
                herd.dwell = 0;
            }
        }

        // desired direction picks the grid step; feeding is decoupled so a travelling elk over food still eats.
        let desired;
        match herd.state {
            HerdState::Graze => {
                // pins the steer (not state) to break the sep/cohesion orbit; leave/cross transitions still run each tick.
                if elk.grazing {
                    desired = Vec2::ZERO;
                } else {
                    let restore = arrive(pos, coh_center, GRAZE_COH_SLOW, GRAZE_COH_DEAD, 1.0) * hp.cohesion;
                    desired = sep + restore + align;
                }
                if herd.dwell >= hp.graze_min_dwell {
                    let here_attract = attract(here, &grid, &ep);
                    let best_frac = best_reachable_food_frac(here, &grid, ep.grass_radius);
                    let depleted = should_leave_patch(here_frac, best_frac, hp.leave_frac, hp.travel_margin);
                    let (cell, val) = richest_within(here, &grid, &ep, hp.scan_radius);
                    let gain = val - here_attract;
                    let leave = (depleted && gain > hp.travel_margin) || gain > hp.pursue_margin;
                    if cell != here && leave {
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
                // steer along the shared gradient (not each elk's private argmax) so the herd travels as a column, not a dispersing spray.
                let goal_dir = signal.normalize_or_zero() * hp.max_speed;
                let follow = coh_dir * (hp.max_speed * hp.cohesion * (1.0 - conf));
                desired = goal_dir * conf + follow + align + sep;

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
                // arrive hard at the far bank, ignoring the herd's backward cohesion pull.
                desired = arrive(pos, target, hp.slow_radius, hp.arrive_radius, hp.max_speed) + sep;
                if grid.water(here) < WATER_EPS && (target - pos).length() < hp.slow_radius {
                    herd.state = HerdState::Graze;
                    herd.goal = Goal::None;
                    herd.dwell = 0;
                }
            }
        }

        // whole-step commit (anti-buzz): elk can't reverse mid-step; renderer lerps prev_cell→cell for smooth motion.
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
                None => elk.move_rate = 0.0,
            }
        }
        if elk.move_t < 1.0 {
            elk.move_t = (elk.move_t + elk.move_rate).min(1.0);
        }

        let water = grid.water(elk.cell);
        if water > 0.0 && !grid.is_ford(elk.cell) {
            let before = elk.energy;
            elk.energy = (elk.energy - ep.swim_drain * water).max(0.0);
            flows.swim += before - elk.energy;
        }
    }
}

// Independent of move state: a grazing elk beside a river still considers the ford on forage merits alone.
fn best_crossing(here: usize, grid: &Grid, ep: &ElkParams) -> Option<usize> {
    let here_attract = attract(here, grid, ep);
    let peek = ep.cross_peek.max(1.0) as usize;
    let mut best: Option<(usize, f32)> = None;
    for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
        let Some(ahead) = grid.step(here, dx, dy) else { continue };
        if grid.water(ahead) < WATER_EPS {
            continue;
        }
        let Some((far, eff)) = forage_across(grid, here, dx, dy, ep.ford_discount, peek) else {
            continue;
        };
        let across = attract(far, grid, ep);
        let cost = swim_cost(eff, ep.swim_reluctance);
        // `ahead` is water (≈ 0 forage); the dry option is staying put.
        let desire = cross_desire(here_attract, here_attract, across, cost);
        if desire > 0.0 && best.is_none_or(|(_, d)| desire > d) {
            best = Some((far, desire));
        }
    }
    best.map(|(far, _)| far)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arrive_full_speed_when_far() {
        let v = arrive(Vec2::ZERO, Vec2::new(10.0, 0.0), 4.0, 0.6, 0.5);
        assert!((v.length() - 0.5).abs() < 1e-6, "should be max speed, got {}", v.length());
        assert!(v.x > 0.0 && v.y.abs() < 1e-6, "should aim +x: {v:?}");
    }

    #[test]
    fn arrive_settles_at_target() {
        let v = arrive(Vec2::ZERO, Vec2::new(0.3, 0.0), 4.0, 0.6, 0.5);
        assert_eq!(v, Vec2::ZERO);
    }

    #[test]
    fn arrive_slows_approaching() {
        let far = arrive(Vec2::ZERO, Vec2::new(3.0, 0.0), 4.0, 0.6, 0.5).length();
        let near = arrive(Vec2::ZERO, Vec2::new(1.5, 0.0), 4.0, 0.6, 0.5).length();
        assert!(near < far, "closer must be slower: {near} !< {far}");
    }

    #[test]
    fn cohesion_target_is_centroid_of_in_range_kin() {
        let kin = [Vec2::new(2.0, 0.0), Vec2::new(0.0, 2.0)];
        let c = cohesion_target(Vec2::ZERO, &kin, Vec2::ZERO, 8.0, 0.0).unwrap();
        assert!((c - Vec2::new(1.0, 1.0)).length() < 1e-6, "centroid expected, got {c:?}");
    }

    #[test]
    fn cohesion_target_falls_back_to_nearest_when_none_in_range() {
        let far = Vec2::new(20.0, 0.0);
        let farther = Vec2::new(40.0, 0.0);
        let c = cohesion_target(Vec2::ZERO, &[farther, far], Vec2::ZERO, 8.0, 0.0).unwrap();
        assert_eq!(c, far, "should steer toward the nearer of two out-of-range kin");
    }

    #[test]
    fn cohesion_target_prefers_in_range_over_fallback() {
        let kin = [Vec2::new(1.0, 0.0), Vec2::new(50.0, 0.0)];
        let c = cohesion_target(Vec2::ZERO, &kin, Vec2::ZERO, 8.0, 0.0).unwrap();
        assert_eq!(c, Vec2::new(1.0, 0.0), "only the in-range kin counts toward the centroid");
    }

    #[test]
    fn cohesion_target_none_without_kin() {
        assert_eq!(cohesion_target(Vec2::ZERO, &[], Vec2::ZERO, 8.0, 0.0), None);
    }

    #[test]
    fn cohesion_target_forward_bias_pulls_toward_kin_ahead() {
        let ahead = Vec2::new(4.0, 0.0);
        let behind = Vec2::new(-4.0, 0.0);
        let plain = cohesion_target(Vec2::ZERO, &[ahead, behind], Vec2::X, 8.0, 0.0).unwrap();
        let biased = cohesion_target(Vec2::ZERO, &[ahead, behind], Vec2::X, 8.0, 1.0).unwrap();
        assert!((plain.x).abs() < 1e-6, "unbiased centroid of symmetric kin is the origin");
        assert!(biased.x > 0.0, "forward bias must shift the target ahead (+x): {biased:?}");
    }

    #[test]
    fn cohesion_target_no_bias_without_heading() {
        let kin = [Vec2::new(4.0, 0.0), Vec2::new(-4.0, 0.0)];
        let c = cohesion_target(Vec2::ZERO, &kin, Vec2::ZERO, 8.0, 1.0).unwrap();
        assert!(c.length() < 1e-6, "no heading ⇒ unbiased centroid, got {c:?}");
    }

    #[test]
    fn confidence_endpoints() {
        assert_eq!(confidence(0.0, 0.5), 0.0);
        assert!((confidence(0.5, 0.5) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn confidence_monotone_in_signal() {
        let lo = confidence(0.2, 0.5);
        let hi = confidence(2.0, 0.5);
        assert!(hi > lo, "stronger signal must raise confidence: {hi} !> {lo}");
        assert!(hi < 1.0, "confidence stays below 1");
    }

    #[test]
    fn quantize_stays_put_under_deadband() {
        assert_eq!(quantize_step(Vec2::new(0.05, 0.0), 0.15), None);
        assert_eq!(quantize_step(Vec2::ZERO, 0.15), None);
    }

    #[test]
    fn quantize_picks_the_nearest_of_eight() {
        assert_eq!(quantize_step(Vec2::new(1.0, 0.0), 0.1), Some((1, 0)));
        assert_eq!(quantize_step(Vec2::new(0.0, 1.0), 0.1), Some((0, 1)));
        assert_eq!(quantize_step(Vec2::new(-1.0, 0.0), 0.1), Some((-1, 0)));
        assert_eq!(quantize_step(Vec2::new(0.0, -1.0), 0.1), Some((0, -1)));
        assert_eq!(quantize_step(Vec2::new(1.0, 1.0), 0.1), Some((1, 1)));
        assert_eq!(quantize_step(Vec2::new(-1.0, -0.9), 0.1), Some((-1, -1)));
    }

    #[test]
    fn quantize_snaps_within_octant() {
        let v = Vec2::new(1.0, 0.36);
        assert_eq!(quantize_step(v, 0.1), Some((1, 0)));
    }

    #[test]
    fn separation_pushes_away_and_ignores_far() {
        let near = separation(Vec2::ZERO, &[Vec2::new(1.0, 0.0)], 2.5);
        assert!(near.x < 0.0, "should push away from the right neighbour: {near:?}");
        let far = separation(Vec2::ZERO, &[Vec2::new(100.0, 0.0)], 2.5);
        assert_eq!(far, Vec2::ZERO, "a neighbour beyond the radius exerts no push");
    }

    #[test]
    fn separation_stronger_when_closer() {
        let close = separation(Vec2::ZERO, &[Vec2::new(0.5, 0.0)], 2.5).length();
        let further = separation(Vec2::ZERO, &[Vec2::new(2.0, 0.0)], 2.5).length();
        assert!(close > further, "closer crowd must push harder: {close} !> {further}");
    }

    #[test]
    fn food_frac_reads_shrubs_where_grass_cannot_grow() {
        let mut grid = Grid::new(5, 5);
        grid.set_water_prox(12, 0.0);
        grid.set_shrub_cap(12, 1.0);
        grid.set_shrubs(12, 0.8);
        assert!((grid.food_frac(12) - 0.8).abs() < 1e-5, "food_frac must reflect shrub fullness");
        grid.eat_shrubs(12, 0.6);
        assert!(grid.food_frac(12) < 0.3, "depleting shrubs must lower food_frac");
    }

    #[test]
    fn food_frac_is_zero_on_carryless_ground() {
        let grid = Grid::new(5, 5);
        assert_eq!(grid.food_frac(12), 0.0);
    }

    #[test]
    fn leaves_only_on_depletion_and_better_reachable() {
        assert!(should_leave_patch(0.2, 0.7, 0.4, 0.05));
        assert!(!should_leave_patch(0.2, 0.22, 0.4, 0.05));
        assert!(!should_leave_patch(0.8, 0.95, 0.4, 0.05));
    }

    #[test]
    fn graze_holds_until_dwell_floor() {
        assert!(!should_leave_graze(10, 30, 0.2, 0.7, 0.4, 0.05));
        assert!(should_leave_graze(30, 30, 0.2, 0.7, 0.4, 0.05));
    }

    #[test]
    fn graze_dwell_floor_does_not_force_a_move() {
        assert!(!should_leave_graze(100, 30, 0.8, 0.95, 0.4, 0.05));
    }
}
