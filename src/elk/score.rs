//! Survival Score — a live rating that rises and falls with recent play, plus the
//! high-water mark it has reached ("your high was 136").
//!
//! Each elk's life ends in a despawn that pays out points: the **forward progress**
//! it made (how far east it got, full credit for a crossing) is the reward, a
//! starvation subtracts a penalty so dying early costs. Each payout is scaled by
//! the **difficulty** the player dialed in (how scarce they made the world) and by
//! a scale that lands strong play in the hundreds. The **current** score is a
//! rolling per-elk average of those payouts — so it moves up and down as the herd
//! does well or badly — and **high** tracks the best current ever reached.
//!
//! Difficulty both scales and *gates* the score: at difficulty 0 every payout is 0,
//! so the only way to a high score is to impose scarcity and still bring the herd
//! through. The decision content is pure (`difficulty`, `despawn_points`, `ewma`)
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
/// Regrowth-ratio at and above which the world comfortably sustains a herd — the
/// zero-difficulty reference. Below it, difficulty climbs toward 1 at total famine.
const REGROW_EASY: f32 = 0.2;
/// Indexes a per-despawn payout (forage progress in [0, 1] × difficulty) into a
/// current score that peaks in the hundreds under strong play at high difficulty.
const POINT_SCALE: f32 = 500.0;

/// Difficulty in `[0, 1]` from the scarcity the player has dialed in: how far the
/// regrowth rate (`regrow_ratio` — forage refill ÷ drain) sits below a comfortably
/// sustainable reference. At the default regrowth it is near zero (easy); as the
/// player starves the world it climbs toward 1 at famine.
pub fn difficulty(regrow_ratio: f32) -> f32 {
    ((REGROW_EASY - regrow_ratio) / REGROW_EASY).clamp(0.0, 1.0)
}

/// Points one despawn pays into the rolling current score. The payout is the
/// elk's *forage progress*: a crossing (`departed`) pays the full `1.0`, a
/// starvation pays how far east it got (`progress = col / edge`). Scaled by
/// `difficulty` (easy worlds pay ~nothing — the gate) and `POINT_SCALE` (lands
/// strong play in the hundreds). Nothing is negative: a herd that dies early just
/// pays little and the rolling score sags; one pushed far across lifts it. Survival
/// is rewarded implicitly — elk kept alive reach higher progress before they go.
pub fn despawn_points(progress: f32, departed: bool, difficulty: f32) -> f32 {
    let credit = if departed { 1.0 } else { progress.clamp(0.0, 1.0) };
    credit * difficulty * POINT_SCALE
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
    let d = difficulty(controls.regrow_ratio);
    score.difficulty = ewma(score.difficulty, d, DIFFICULTY_ALPHA);

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
            let payout = despawn_points(progress, departed, score.difficulty);
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

    // Payout rises with progress; a crossing pays the most, an early death the
    // least (but never negative) — so the score rewards forward progress.
    #[test]
    fn payout_rises_with_progress() {
        let d = 1.0;
        assert!(despawn_points(0.1, false, d) >= 0.0, "an early death pays little, not negative");
        assert!(
            despawn_points(0.9, false, d) > despawn_points(0.1, false, d),
            "dying farther across pays more"
        );
        assert!(
            despawn_points(0.95, false, d) < despawn_points(0.95, true, d),
            "crossing alive beats starving just short"
        );
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
}
