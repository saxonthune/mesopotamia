//! Vegetation layer: shrub (big-leaf) carrying capacity. The densest noise clumps
//! settle on dry ground far from water and are excluded from water cells — "densest"
//! meaning a per-world quantile cut sized so a target *fraction* of the eligible
//! steppe bears shrubs, which keeps coverage in a controlled band instead of letting
//! it swing seed to seed with the noise distribution's shape. Reads the
//! water field the water layer already laid down — and the rough layer placed just
//! before it: shrubs grow *around* rough terrain, leaving a blank border so the two
//! read as distinct features rather than overlapping into camouflage. The runtime
//! shrub regrowth (in `grid`) grows the standing crop toward this capacity.

use rand::Rng;
use rand::SeedableRng;
use rand::rngs::StdRng;

use crate::field;
use crate::grid::{Grid, MAX_SHRUBS};

const SHRUB_PASSES: usize = 5;
/// Extra horizontal-only smoothing passes applied to the shrub noise, stretching
/// the clumps east-west into a lateral forage grain the herd can graze along
/// between the (roughly vertical) rivers. Subtle: at this level shrubs read as
/// laterally-leaning patches, not stripes. Coverage is unchanged — the quantile
/// cut in `coverage_threshold` keeps the same fraction of steppe regardless of
/// clump shape. Raise it for a stronger grain; 0 restores the old round clumps.
const SHRUB_X_STRETCH: usize = 4;
/// Densest worlds: this fraction of the eligible steppe (non-water, non-rough)
/// bears shrubs — the calibrated *max* coverage. The cut is taken as a per-world
/// quantile of the noise (see `coverage_threshold`) rather than a fixed value, so
/// coverage no longer swings seed to seed with the noise distribution's shape.
const SHRUB_COVER_MAX: f32 = 0.50;
/// Sparsest worlds: the lower bound on coverage, 60% of the max. Each world draws
/// its target coverage uniformly in `[SHRUB_COVER_FLOOR, SHRUB_COVER_MAX]`.
const SHRUB_COVER_FLOOR: f32 = 0.30;
/// Per-tile probability that a shrub-bearing cell bears a white flower. Sparse so
/// flowers read as scattered accents rather than a carpet.
const FLOWER_CHANCE: f32 = 0.08;
/// Blank border, in cells, kept clear of shrubs around every rough patch. Shrubs
/// fill the space the rough leaves, set back by this margin so each rough glyph
/// reads with a clean halo rather than blending into the vegetation.
const SHRUB_ROUGH_MARGIN: i32 = 1;
/// Soil-type cutoff above which a cell reads as wet riparian ground and bears no
/// shrubs at all. Shrubs are dry-steppe forage, so the moist band hugging water is
/// excluded outright rather than merely thinned — the visible dark tiles stay clear.
const SHRUB_WET_SOIL_MAX: f32 = 0.5;

/// A cell can bear shrubs only on dry steppe clear of the rough border: no standing
/// water, soil-type below the riparian (wet) cutoff, and not within the rough
/// margin. Pure so the water/wet/rough gate is pinned by a test rather than read
/// off the screen.
fn shrub_eligible(water: f32, soil_type: f32, blocked: bool) -> bool {
    water <= 0.0 && soil_type < SHRUB_WET_SOIL_MAX && !blocked
}

/// Dilate a rough-presence mask by `margin` cells (8-connected): a cell is blocked
/// if it is rough or lies within `margin` of a rough cell. Pure so the border
/// contract is pinned by a test rather than read off the screen.
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

/// Pick the patch-value cut that lets the top `target` fraction of *eligible*
/// cells through (`patch >= threshold` is kept). Replacing the old fixed cut, this
/// keys the shrub coverage to a controlled fraction of the eligible steppe rather
/// than to whatever fraction of a per-world noise field happens to clear a constant
/// — which is what made coverage swing seed to seed. Pure so the contract is pinned
/// by a test.
fn coverage_threshold(patch: &[f32], eligible: &[bool], target: f32) -> f32 {
    let mut vals: Vec<f32> = patch
        .iter()
        .zip(eligible)
        .filter(|&(_, &e)| e)
        .map(|(&p, _)| p)
        .collect();
    if vals.is_empty() {
        return f32::INFINITY; // nothing eligible → keep nothing
    }
    vals.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let target = target.clamp(0.0, 1.0);
    // Keep the top `target` fraction: cut at the (1 - target) quantile.
    let rank = (((1.0 - target) * vals.len() as f32).floor() as usize).min(vals.len() - 1);
    vals[rank]
}

/// Author shrub capacity: dense noise clumps on dry ground (far from water),
/// excluded from water cells, from the wet riparian band (`soil_type` above the
/// wet cutoff), and from rough terrain plus a `SHRUB_ROUGH_MARGIN` border. Reads
/// `water`/`water_prox`/`soil_type`/`rough` straight off the grid, which the
/// earlier layers have already filled by the time this layer runs.
///
/// `shrub_seed` is distinct from the soil and river seeds so the clumps fall
/// independently of where water and good grazing land are; the orchestrator
/// derives it from the master world seed.
pub(super) fn seed_shrub_cap(grid: &mut Grid, shrub_seed: u64) {
    let mut rng = StdRng::seed_from_u64(shrub_seed);
    // Isotropic value-noise clumps, then horizontal-only passes to lean them
    // east-west — a lateral forage grain. The quantile coverage cut below is
    // shape-agnostic, so this elongates the clumps without thinning the shrubs.
    let raw = field::value_noise(grid.width(), grid.height(), SHRUB_PASSES, &mut rng);
    let stretched = field::smooth_axis(&raw, grid.width(), grid.height(), SHRUB_X_STRETCH, true);
    let patch = field::normalize(&stretched);
    // Shrubs grow around the rough glyphs: block rough cells and the border ring.
    let rough: Vec<bool> = (0..grid.len()).map(|i| grid.rough(i) > 0.0).collect();
    let blocked = rough_blocked_mask(&rough, grid.width(), grid.height(), SHRUB_ROUGH_MARGIN);
    // A cell can bear a shrub only if it is neither water nor rough-blocked. Draw
    // this world's target coverage from the calibrated band, then pick the noise cut
    // that lets that fraction of the eligible steppe through — so coverage lands in
    // [floor, max] every world instead of swinging with the noise distribution.
    let eligible: Vec<bool> = (0..grid.len())
        .map(|i| shrub_eligible(grid.water(i), grid.soil_type(i), blocked[i]))
        .collect();
    let target = rng.random_range(SHRUB_COVER_FLOOR..=SHRUB_COVER_MAX);
    let threshold = coverage_threshold(&patch, &eligible, target);
    for (i, &p) in patch.iter().enumerate() {
        // Dryness: far from water → 1, beside it → 0. Shrubs want the steppe.
        let dry = 1.0 - grid.water_prox(i);
        // Gentle nudge: riparian cells (soil_type → 1) slightly suppress shrubs.
        let steppe = 1.0 - 0.5 * grid.soil_type(i);
        let cap = if !eligible[i] || p < threshold {
            0.0
        } else {
            dry * steppe * MAX_SHRUBS
        };
        grid.set_shrub_cap(i, cap);
        // Seed the standing crop at full capacity so the world is generated with
        // mature shrubs already in place — flowers can bloom on the first tick
        // rather than waiting for regrowth to climb from bare ground.
        grid.set_shrubs(i, cap);
        // A random scatter of shrub cells bear a flower (only where a shrub can
        // actually grow). Bloom — and colour — are decided at render time.
        grid.set_flower(i, cap > 0.0 && rng.random::<f32>() < FLOWER_CHANCE);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wet_tiles_bear_no_shrubs() {
        // Dry steppe clear of rough is eligible.
        assert!(shrub_eligible(0.0, 0.0, false), "dry steppe should bear shrubs");
        // A wet riparian tile (soil_type at/above the cutoff) is excluded even when
        // it holds no standing water itself.
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
        // A single rough cell at the center of a 7×7 grid blocks itself and every
        // cell within Chebyshev distance `margin`, but nothing beyond it.
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
        // Ten eligible cells with values 0.0..0.9; asking for the top 30% must cut
        // so that exactly the three highest (0.7, 0.8, 0.9) clear the threshold.
        let patch: Vec<f32> = (0..10).map(|i| i as f32 / 10.0).collect();
        let eligible = vec![true; 10];
        let t = coverage_threshold(&patch, &eligible, 0.3);
        let kept = patch.iter().filter(|&&p| p >= t).count();
        assert_eq!(kept, 3, "top 30% of ten eligible cells is three");
    }

    #[test]
    fn coverage_threshold_ignores_ineligible_cells() {
        // Ineligible cells (water/rough) must not enter the quantile: only the four
        // eligible values count, so the top 50% is the two highest of those four.
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
