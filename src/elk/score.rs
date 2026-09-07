//! Rolling survival score. Difficulty gates payouts (easy world → zero); pull penalty
//! discounts crossings bought by cranking `cross_ratio` instead of foraging across.

use bevy::prelude::*;

use crate::events::{EventKind, EventLog};
use crate::grid::GRID_WIDTH;

use super::ratios::RatioControls;
use super::EDGE_COL;

const DIFFICULTY_ALPHA: f32 = 0.03;
/// Responsive over last several elk to leave the world.
const RATING_ALPHA: f32 = 0.12;
/// Regrowth at or above which difficulty is 0; below it difficulty climbs to 1 at famine.
const REGROW_EASY: f32 = 0.023;
/// Scales payout into the hundreds under strong play at high difficulty.
const POINT_SCALE: f32 = 500.0;
/// Pull-bought crossing keeps `1 − PULL_PENALTY` credit — costly but not worthless.
const PULL_PENALTY: f32 = 0.6;
/// `cross_ratio` at which pull penalty saturates; `p = (cross_ratio / CROSS_REF).clamp(0,1)`.
const CROSS_REF: f32 = 2.0;
/// Subtracted from a starvation's progress; ensures an early death goes net-negative.
const STARVE_PENALTY: f32 = 0.5;

pub fn difficulty(grass_regrow: f32) -> f32 {
    ((REGROW_EASY - grass_regrow) / REGROW_EASY).clamp(0.0, 1.0)
}

pub fn despawn_points(progress: f32, departed: bool, difficulty: f32) -> f32 {
    let credit = if departed {
        1.0
    } else {
        progress.clamp(0.0, 1.0) - STARVE_PENALTY
    };
    credit * difficulty * POINT_SCALE
}

/// Clamped to `[1 − penalty, 1]`: pull reliance costs score but never scores negative.
pub fn pull_factor(pull_share: f32, penalty: f32) -> f32 {
    (1.0 - penalty * pull_share.clamp(0.0, 1.0)).clamp(1.0 - penalty, 1.0)
}

pub fn ewma(prev: f32, sample: f32, alpha: f32) -> f32 {
    prev + alpha * (sample - prev)
}

#[derive(Resource, Default)]
pub struct Score {
    pub current: f32,
    pub high: f32,
    pub difficulty: f32,
    cursor: u64, // events already folded; monotonic cursor into EventLog.total
}

pub(super) fn update_score(
    mut score: ResMut<Score>,
    events: Res<EventLog>,
    controls: Res<RatioControls>,
) {
    let d = difficulty(controls.grass_regrow);
    score.difficulty = ewma(score.difficulty, d, DIFFICULTY_ALPHA);

    let p = (controls.cross_ratio / CROSS_REF).clamp(0.0, 1.0);

    // Monotonic total survives ring eviction, so each despawn scores exactly once.
    let new = events.total.saturating_sub(score.cursor) as usize;
    if new > 0 {
        let edge = EDGE_COL.max(1) as f32;
        let take = new.min(events.recent.len());
        let start = events.recent.len() - take;
        for e in events.recent.iter().skip(start) {
            let progress = (e.cell % GRID_WIDTH) as f32 / edge;
            let departed = matches!(e.kind, EventKind::Departed);
            let payout = despawn_points(progress, departed, score.difficulty)
                * pull_factor(p, PULL_PENALTY);
            score.current = ewma(score.current, payout, RATING_ALPHA);
            score.high = score.high.max(score.current);
        }
        score.cursor = events.total;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn difficulty_maps_regrow_ratio_to_unit_range() {
        assert_eq!(difficulty(REGROW_EASY), 0.0, "comfortable regrowth → no difficulty");
        assert_eq!(difficulty(0.5), 0.0, "abundant clamps to 0");
        assert!((difficulty(REGROW_EASY / 2.0) - 0.5).abs() < 1e-6, "half the reference → mid difficulty");
        assert_eq!(difficulty(0.0), 1.0, "no regrowth → full difficulty");
    }

    #[test]
    fn easy_world_pays_nothing() {
        assert_eq!(despawn_points(1.0, true, 0.0), 0.0, "a crossing on easy mode pays 0");
    }

    #[test]
    fn payout_rises_with_progress() {
        let d = 1.0;
        assert!(
            despawn_points(0.9, false, d) > despawn_points(0.1, false, d),
            "dying farther across pays more"
        );
        assert!(
            despawn_points(0.95, false, d) < despawn_points(0.95, true, d),
            "crossing alive beats starving just short"
        );
    }

    #[test]
    fn starvation_costs_against_crossing() {
        let d = 1.0;
        // A far-bank death is worth far less than a crossing at the same column.
        let far_death = despawn_points(0.94, false, d);
        let crossing = despawn_points(0.94, true, d);
        // Breaking input: drop STARVE_PENALTY to 0 and both assertions fail.
        assert!(crossing > far_death * 2.0, "crossing alive must dwarf a far-bank death");
        assert!(despawn_points(0.1, false, d) < 0.0, "an early death sinks the score");
    }

    #[test]
    fn difficulty_scales_the_payout() {
        let easy = despawn_points(1.0, true, 0.2);
        let hard = despawn_points(1.0, true, 0.8);
        assert!(hard > easy * 3.0, "4× the difficulty pays ~4× the points");
    }

    #[test]
    fn high_holds_the_peak() {
        let mut high = 0.0_f32;
        let mut current = 0.0_f32;
        for sample in [50.0_f32, 136.0, 20.0, -10.0] {
            current = ewma(current, sample, 1.0); // alpha 1 → current == sample
            high = high.max(current);
        }
        assert_eq!(high, 136.0, "high holds the peak through the later slump");
        assert_eq!(current, -10.0, "current follows the latest payout");
    }

    #[test]
    fn ewma_relaxes_toward_the_sample() {
        assert!((ewma(0.0, 1.0, 0.5) - 0.5).abs() < 1e-6);
        assert!((ewma(2.0, 2.0, 0.3) - 2.0).abs() < 1e-6, "agreement is a fixed point");
    }

    #[test]
    fn pull_factor_endpoints() {
        assert!((pull_factor(0.0, 0.6) - 1.0).abs() < 1e-6, "zero pull share → full credit");
        assert!((pull_factor(1.0, 0.6) - 0.4).abs() < 1e-6, "full pull share → 1 − penalty");
    }

    #[test]
    fn pull_factor_monotone_decreasing() {
        let shares = [0.0_f32, 0.25, 0.5, 0.75, 1.0];
        let factors: Vec<f32> = shares.iter().map(|&s| pull_factor(s, 0.6)).collect();
        // Breaking input: if the factor rose with share, this would fail.
        for w in factors.windows(2) {
            assert!(w[0] >= w[1], "factor must not rise as pull_share increases: {} < {}", w[0], w[1]);
        }
    }

    #[test]
    fn pull_factor_bounded() {
        let penalty = 0.6_f32;
        for share in [-0.5_f32, 0.0, 0.5, 1.0, 1.5, 2.0] {
            let f = pull_factor(share, penalty);
            assert!(f >= 1.0 - penalty - 1e-6, "factor below floor at share={}: {}", share, f);
            assert!(f <= 1.0 + 1e-6, "factor above ceiling at share={}: {}", share, f);
        }
    }

    #[test]
    fn payout_falls_with_pull_reliance() {
        let progress = 1.0_f32;
        let diff = 0.8_f32;
        let base = despawn_points(progress, true, diff);
        let foraged = base * pull_factor(0.0, PULL_PENALTY);
        let pulled = base * pull_factor(0.8, PULL_PENALTY);
        assert!(foraged > pulled, "foraged crossing must outscore a pull-bought one");
    }

    #[test]
    fn payout_falls_as_cross_ratio_rises() {
        let progress = 1.0_f32;
        let diff = 0.8_f32;
        let base = despawn_points(progress, true, diff);

        let payout_at = |cross_ratio: f32| {
            let p = (cross_ratio / CROSS_REF).clamp(0.0, 1.0);
            base * pull_factor(p, PULL_PENALTY)
        };

        let zero = payout_at(0.0);
        let low = payout_at(0.5);
        let mid = payout_at(1.0);
        let high = payout_at(2.0);

        assert!(zero > low, "cross_ratio=0 must score more than cross_ratio=0.5");
        assert!(low > mid, "cross_ratio=0.5 must score more than cross_ratio=1.0");
        assert!(mid > high, "cross_ratio=1.0 must score more than cross_ratio=2.0");
    }

    #[test]
    fn zero_cross_ratio_is_unpenalised() {
        let p = (0.0_f32 / CROSS_REF).clamp(0.0, 1.0);
        assert!((pull_factor(p, PULL_PENALTY) - 1.0).abs() < 1e-6, "zero pull must not penalise");
    }
}
