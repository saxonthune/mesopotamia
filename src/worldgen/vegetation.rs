//! Vegetation layer: shrub carrying capacity on dry steppe, placed around rough glyphs with a
//! clear margin. Coverage is a per-world quantile cut, not a fixed noise threshold.

use rand::Rng;
use rand::SeedableRng;
use rand::rngs::StdRng;

use crate::field;
use crate::grid::{Grid, MAX_SHRUBS};

const SHRUB_PASSES: usize = 5;
/// Horizontal-only passes that elongate clumps east-west without changing coverage (quantile cut is shape-agnostic).
const SHRUB_X_STRETCH: usize = 4;
/// Max coverage fraction; quantile cut keeps this stable across seeds.
const SHRUB_COVER_MAX: f32 = 0.50;
const SHRUB_COVER_FLOOR: f32 = 0.30;
const FLOWER_CHANCE: f32 = 0.08;
const SHRUB_ROUGH_MARGIN: i32 = 1;
/// Riparian cutoff: cells above this soil_type are excluded outright (not thinned).
const SHRUB_WET_SOIL_MAX: f32 = 0.5;

fn shrub_eligible(water: f32, soil_type: f32, blocked: bool) -> bool {
    water <= 0.0 && soil_type < SHRUB_WET_SOIL_MAX && !blocked
}

fn rough_blocked_mask(rough: &[bool], width: usize, height: usize, margin: i32) -> Vec<bool> {
    let mut blocked = rough.to_vec();
    for _ in 0..margin {
        let prev = blocked.clone();
        for row in 0..height {
            for col in 0..width {
                let i = row * width + col;
                if prev[i] {
                    continue;
                }
                'expand: for dy in -1..=1i32 {
                    for dx in -1..=1i32 {
                        let nc = col as i32 + dx;
                        let nr = row as i32 + dy;
                        if nc < 0 || nr < 0 || nc >= width as i32 || nr >= height as i32 {
                            continue;
                        }
                        if prev[nr as usize * width + nc as usize] {
                            blocked[i] = true;
                            break 'expand;
                        }
                    }
                }
            }
        }
    }
    blocked
}

/// Quantile cut: lets the top `target` fraction of eligible cells through — coverage stays stable across seeds.
fn coverage_threshold(patch: &[f32], eligible: &[bool], target: f32) -> f32 {
    let mut vals: Vec<f32> = patch
        .iter()
        .zip(eligible)
        .filter(|&(_, &e)| e)
        .map(|(&p, _)| p)
        .collect();
    if vals.is_empty() {
        return f32::INFINITY;
    }
    vals.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let target = target.clamp(0.0, 1.0);
    let rank = (((1.0 - target) * vals.len() as f32).floor() as usize).min(vals.len() - 1);
    vals[rank]
}

pub(super) fn seed_shrub_cap(grid: &mut Grid, shrub_seed: u64) {
    let mut rng = StdRng::seed_from_u64(shrub_seed);
    let raw = field::value_noise(grid.width(), grid.height(), SHRUB_PASSES, &mut rng);
    let stretched = field::smooth_axis(&raw, grid.width(), grid.height(), SHRUB_X_STRETCH, true);
    let patch = field::normalize(&stretched);
    let rough: Vec<bool> = (0..grid.len()).map(|i| grid.rough(i) > 0.0).collect();
    let blocked = rough_blocked_mask(&rough, grid.width(), grid.height(), SHRUB_ROUGH_MARGIN);
    let eligible: Vec<bool> = (0..grid.len())
        .map(|i| shrub_eligible(grid.water(i), grid.soil_type(i), blocked[i]))
        .collect();
    let target = rng.random_range(SHRUB_COVER_FLOOR..=SHRUB_COVER_MAX);
    let threshold = coverage_threshold(&patch, &eligible, target);
    for (i, &p) in patch.iter().enumerate() {
        let dry = 1.0 - grid.water_prox(i);
        let steppe = 1.0 - 0.5 * grid.soil_type(i);
        let cap = if !eligible[i] || p < threshold {
            0.0
        } else {
            dry * steppe * MAX_SHRUBS
        };
        grid.set_shrub_cap(i, cap);
        grid.set_shrubs(i, cap); // mature shrubs on day one so flowers can bloom immediately
        grid.set_flower(i, cap > 0.0 && rng.random::<f32>() < FLOWER_CHANCE);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wet_tiles_bear_no_shrubs() {
        assert!(shrub_eligible(0.0, 0.0, false), "dry steppe should bear shrubs");
        assert!(
            !shrub_eligible(0.0, SHRUB_WET_SOIL_MAX, false),
            "wet riparian tile beside water should bear no shrubs"
        );
        assert!(!shrub_eligible(0.0, 0.9, false), "very wet tile is excluded");
        // Water cells and rough-bordered cells stay excluded too.
        assert!(!shrub_eligible(0.5, 0.0, false), "water cell is excluded");
        assert!(!shrub_eligible(0.0, 0.0, true), "rough-bordered cell is excluded");
    }

    #[test]
    fn rough_border_is_cleared() {
        let (w, h) = (7usize, 7usize);
        let mut rough = vec![false; w * h];
        let center = 3 * w + 3;
        rough[center] = true;

        let blocked = rough_blocked_mask(&rough, w, h, 2);

        assert!(blocked[center], "the rough cell itself is blocked");
        assert!(blocked[3 * w + 5], "a cell 2 away (within margin) is blocked");
        assert!(!blocked[3 * w + 6], "a cell 3 away (beyond margin) is clear");
        assert!(!blocked[0], "a far corner is clear");
    }

    #[test]
    fn coverage_threshold_keeps_the_target_fraction() {
        let patch: Vec<f32> = (0..10).map(|i| i as f32 / 10.0).collect();
        let eligible = vec![true; 10];
        let t = coverage_threshold(&patch, &eligible, 0.3);
        let kept = patch.iter().filter(|&&p| p >= t).count();
        assert_eq!(kept, 3, "top 30% of ten eligible cells is three");
    }

    #[test]
    fn coverage_threshold_ignores_ineligible_cells() {
        let patch = vec![0.9, 0.1, 0.8, 0.2, 0.3, 0.4];
        let eligible = vec![false, true, false, true, true, true]; // values 0.1..0.4
        let t = coverage_threshold(&patch, &eligible, 0.5);
        let kept = patch
            .iter()
            .zip(&eligible)
            .filter(|&(&p, &e)| e && p >= t)
            .count();
        assert_eq!(kept, 2, "top 50% of the four eligible cells is two");
    }

    #[test]
    fn coverage_threshold_no_eligible_keeps_nothing() {
        let t = coverage_threshold(&[0.5, 0.6], &[false, false], 0.5);
        assert!(t.is_infinite(), "no eligible cell → threshold nothing can clear");
    }

    #[test]
    fn no_rough_blocks_nothing() {
        let (w, h) = (5usize, 5usize);
        let blocked = rough_blocked_mask(&vec![false; w * h], w, h, 2);
        assert!(blocked.iter().all(|&b| !b), "with no rough, no cell is blocked");
    }
}
