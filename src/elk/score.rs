//! Survival Score — a live rating that rises and falls with recent play, plus the
//! high-water mark it has reached ("your high was 136").
//!
//! Each elk's life ends in a despawn that pays out points: the **forward progress**
//! it made (how far east it got, full credit for a crossing) is the reward, a
//! starvation subtracts a penalty so dying early costs. Each payout is scaled by
//! the **difficulty** the player dialed in (how scarce they made the world), by a
//! **pull penalty** that discounts crossings bought with the migration force, and by
//! a scale that lands strong play in the hundreds. The **current** score is a
//! rolling per-elk average of those payouts — so it moves up and down as the herd
//! does well or badly — and **high** tracks the best current ever reached.
//!
//! Difficulty both scales and *gates* the score: at difficulty 0 every payout is 0,
//! so the only way to a high score is to impose scarcity and still bring the herd
//! through. The pull penalty is orthogonal: a crossing earned by letting the herd
//! forage its way across scores full credit; one bought by cranking the dialed
//! migration ÷ crossing-cost slider (`cross_ratio`) scores only a fraction. The
//! decision content is pure (`difficulty`, `despawn_points`, `pull_factor`, `ewma`)
//! and unit-tested; the system only gathers state and folds events through it.

use bevy::prelude::*;

use crate::events::{EventKind, EventLog};
use crate::grid::GRID_WIDTH;

use super::ratios::RatioControls;
use super::EDGE_COL;

/// EWMA weight on the per-tick difficulty sample — slow, so the gauge is steady.
const DIFFICULTY_ALPHA: f32 = 0.03;
/// EWMA weight on each despawn payout — the current score's rolling-per-elk feel,
/// responsive over the last several elk to leave the world.
const RATING_ALPHA: f32 = 0.12;
/// Grass regrowth at and above which the world comfortably sustains a herd — the
/// zero-difficulty reference. Below it, difficulty climbs toward 1 at total famine.
const REGROW_EASY: f32 = 0.023;
/// Indexes a per-despawn payout (forage progress in [0, 1] × difficulty) into a
/// current score that peaks in the hundreds under strong play at high difficulty.
const POINT_SCALE: f32 = 500.0;
/// Maximum score discount for relying on the migration pull. At full reliance
/// (`pull_share == 1`) the payout is `1 − PULL_PENALTY` of the natural-drive payout.
/// Start at 0.6 so a pull-bought crossing keeps 40% of the credit — punishing but
/// not worthless, leaving the pull usable as a rescue tool at a score cost.
const PULL_PENALTY: f32 = 0.6;
/// The dialed `cross_ratio` level at which the pull penalty saturates (≈ slider max).
/// Normalised: `p = (cross_ratio / CROSS_REF).clamp(0, 1)` — so at `cross_ratio == 0`
/// the factor is 1 (unpenalised) and at `cross_ratio == CROSS_REF` it is at full bite.
const CROSS_REF: f32 = 2.0;
/// Credit subtracted from a starvation's forage progress — the cost of *dying* as
/// opposed to *crossing*. Without it, an elk that starves at the far bank (progress
/// ≈ 0.94) scores almost as much as one that crosses alive (1.0), so a herd that
/// marches east and dies en masse rates nearly a clean run. At 0.5 a far-bank death
/// pays ~0.44 against a crossing's 1.0 (crossing alive ≈ 2.3× a death at the same
/// column), and an *early* death goes net-negative — so a wipeout actively sinks the
/// score. This is what makes a high score mean the herd is *surviving*, not just
/// reaching far before it dies.
const STARVE_PENALTY: f32 = 0.5;

/// Difficulty in `[0, 1]` from the scarcity the player has dialed in: how far the
/// grass regrowth rate sits below a comfortably sustainable reference. At the default
/// regrowth it is near zero (easy); as the player starves the world it climbs toward
/// 1 at famine.
pub fn difficulty(grass_regrow: f32) -> f32 {
    ((REGROW_EASY - grass_regrow) / REGROW_EASY).clamp(0.0, 1.0)
}

/// Points one despawn pays into the rolling current score. A crossing (`departed`)
/// pays full `1.0`; a starvation pays how far east it got *minus a death penalty*
/// (`progress − STARVE_PENALTY`), so dying costs against crossing — and an early
/// death goes net-negative and sinks the rolling score. Scaled by `difficulty`
/// (easy worlds pay ~nothing — the gate) and `POINT_SCALE` (lands strong play in
/// the hundreds). This is what makes a high score mean the herd *survives*: a herd
/// that marches far east and starves en masse rates well below one that crosses alive.
pub fn despawn_points(progress: f32, departed: bool, difficulty: f32) -> f32 {
    let credit = if departed {
        1.0
    } else {
        progress.clamp(0.0, 1.0) - STARVE_PENALTY
    };
    credit * difficulty * POINT_SCALE
}

/// Score multiplier for how the crossing was earned. At `pull_share == 0` (all natural
/// drive) the factor is 1 — full credit. At `pull_share == 1` (all magic pull) it falls
/// to `1 − penalty`. Linear and clamped to `[1 − penalty, 1]`: relying on the pull costs
/// score, but a pull-assisted crossing is never worthless and never scores negative.
pub fn pull_factor(pull_share: f32, penalty: f32) -> f32 {
    (1.0 - penalty * pull_share.clamp(0.0, 1.0)).clamp(1.0 - penalty, 1.0)
}

/// One exponential-moving-average step: `prev` relaxed toward `sample` by `alpha`.
pub fn ewma(prev: f32, sample: f32, alpha: f32) -> f32 {
    prev + alpha * (sample - prev)
}

/// The live demo score, read by the UI gauges.
#[derive(Resource, Default)]
pub struct Score {
    /// Live rating: rolling per-elk average of despawn payouts. Rises and falls
    /// with recent play; can go negative when the herd dies for nothing.
    pub current: f32,
    /// High-water mark — the best `current` reached this session.
    pub high: f32,
    /// Smoothed current difficulty in `[0, 1]` — the gate, shown alongside.
    pub difficulty: f32,
    /// Events already folded into the score — a cursor into `EventLog.total`.
    cursor: u64,
}

/// Update the difficulty (from the dialed scarcity) and fold any despawn events
/// since last tick into the rolling current score, tracking the high-water mark.
pub(super) fn update_score(
    mut score: ResMut<Score>,
    events: Res<EventLog>,
    controls: Res<RatioControls>,
) {
    let d = difficulty(controls.grass_regrow);
    score.difficulty = ewma(score.difficulty, d, DIFFICULTY_ALPHA);

    // Normalised dialed pull in [0, 1] — the penalty argument for the score multiplier.
    // Keyed off the `cross_ratio` slider, so cranking the migration ÷ crossing-cost
    // dial immediately costs score.
    let p = (controls.cross_ratio / CROSS_REF).clamp(0.0, 1.0);

    // Fold the events pushed since our cursor — newest `new` entries in the ring.
    // The monotonic `total` survives ring eviction, so each despawn scores once.
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

    // Difficulty is 0 at/above the sustainable regrowth reference and rises to 1
    // as the dialed regrowth falls to nothing — the gate the score scales by.
    #[test]
    fn difficulty_maps_regrow_ratio_to_unit_range() {
        assert_eq!(difficulty(REGROW_EASY), 0.0, "comfortable regrowth → no difficulty");
        assert_eq!(difficulty(0.5), 0.0, "abundant clamps to 0");
        assert!((difficulty(REGROW_EASY / 2.0) - 0.5).abs() < 1e-6, "half the reference → mid difficulty");
        assert_eq!(difficulty(0.0), 1.0, "no regrowth → full difficulty");
    }

    // An easy world (difficulty 0) pays nothing, however far the herd gets — the
    // gate that forces the player to impose scarcity to build a score.
    #[test]
    fn easy_world_pays_nothing() {
        assert_eq!(despawn_points(1.0, true, 0.0), 0.0, "a crossing on easy mode pays 0");
    }

    // Payout rises with progress; a crossing pays the most, dying farther east beats
    // dying early, and a crossing always beats starving at the same column.
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

    // The death penalty: a crossing outscores a starvation at the *same* column by a
    // wide margin, and an early death goes net-negative — a wipeout sinks the score.
    // Breaking input: drop STARVE_PENALTY to 0 and both assertions fail.
    #[test]
    fn starvation_costs_against_crossing() {
        let d = 1.0;
        // A far-bank death is worth far less than a crossing at the same column.
        let far_death = despawn_points(0.94, false, d);
        let crossing = despawn_points(0.94, true, d);
        assert!(crossing > far_death * 2.0, "crossing alive must dwarf a far-bank death");
        // An early death is a net loss, not a small gain.
        assert!(despawn_points(0.1, false, d) < 0.0, "an early death sinks the score");
    }

    // Difficulty scales the payout: the same outcome pays more under scarcity,
    // which is how a high score is gated to hard play.
    #[test]
    fn difficulty_scales_the_payout() {
        let easy = despawn_points(1.0, true, 0.2);
        let hard = despawn_points(1.0, true, 0.8);
        assert!(hard > easy * 3.0, "4× the difficulty pays ~4× the points");
    }

    // The high-water mark holds at the peak even as the current score falls back —
    // "your high was 136" survives a later slump.
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

    // EWMA moves toward the sample and is a fixed point at agreement.
    #[test]
    fn ewma_relaxes_toward_the_sample() {
        assert!((ewma(0.0, 1.0, 0.5) - 0.5).abs() < 1e-6);
        assert!((ewma(2.0, 2.0, 0.3) - 2.0).abs() < 1e-6, "agreement is a fixed point");
    }

    // pull_factor endpoints: no reliance → full credit; full reliance → 1 − penalty.
    #[test]
    fn pull_factor_endpoints() {
        assert!((pull_factor(0.0, 0.6) - 1.0).abs() < 1e-6, "zero pull share → full credit");
        assert!((pull_factor(1.0, 0.6) - 0.4).abs() < 1e-6, "full pull share → 1 − penalty");
    }

    // pull_factor is monotone decreasing: more reliance on the pull never raises
    // the factor. Breaking input: if the factor rose with share, this would fail.
    #[test]
    fn pull_factor_monotone_decreasing() {
        let shares = [0.0_f32, 0.25, 0.5, 0.75, 1.0];
        let factors: Vec<f32> = shares.iter().map(|&s| pull_factor(s, 0.6)).collect();
        for w in factors.windows(2) {
            assert!(w[0] >= w[1], "factor must not rise as pull_share increases: {} < {}", w[0], w[1]);
        }
    }

    // pull_factor is bounded in [1 − penalty, 1] including out-of-range shares.
    #[test]
    fn pull_factor_bounded() {
        let penalty = 0.6_f32;
        for share in [-0.5_f32, 0.0, 0.5, 1.0, 1.5, 2.0] {
            let f = pull_factor(share, penalty);
            assert!(f >= 1.0 - penalty - 1e-6, "factor below floor at share={}: {}", share, f);
            assert!(f <= 1.0 + 1e-6, "factor above ceiling at share={}: {}", share, f);
        }
    }

    // A pull-bought crossing scores strictly less than a foraged one. This is the
    // headline property the penalty exists to enforce.
    #[test]
    fn payout_falls_with_pull_reliance() {
        let progress = 1.0_f32;
        let diff = 0.8_f32;
        let base = despawn_points(progress, true, diff);
        let foraged = base * pull_factor(0.0, PULL_PENALTY);
        let pulled = base * pull_factor(0.8, PULL_PENALTY);
        assert!(foraged > pulled, "foraged crossing must outscore a pull-bought one");
    }

    // Payout via the dialed cross_ratio falls as cross_ratio rises.
    // This is the property the re-keyed penalty exists to enforce:
    // cranking the pull slider costs score immediately.
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

    // At cross_ratio == 0 the payout is unpenalised (factor == 1).
    #[test]
    fn zero_cross_ratio_is_unpenalised() {
        let p = (0.0_f32 / CROSS_REF).clamp(0.0, 1.0);
        assert!((pull_factor(p, PULL_PENALTY) - 1.0).abs() < 1e-6, "zero pull must not penalise");
    }
}
