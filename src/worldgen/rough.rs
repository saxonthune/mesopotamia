//! Rough-terrain layer: discrete placed strokes of broken ground. Instead of
//! thresholding a noise field (which only yields patches), this layer PLACES
//! units — small crescent/tilde glyphs — at blue-noise-spaced origins on the dry
//! steppe, each one a parametric curve rasterized to cells. This is the
//! Derive→Distribute shift the methodology calls for (doc01.04, doc02.02): the
//! stroke is the discrete-feature shape operator, best-candidate the void-aware
//! placement.
//!
//! Origins are spaced by Mitchell's best-candidate over candidate cells that are
//! dry (away from water — strokes belong on the steppe) and never on water. Each
//! stroke's length, amplitude, angle jitter, and curve count (crescent vs tilde)
//! are drawn per-mark from the seeded rng so the scatter reads varied. Reads the
//! water field the water layer already laid down, which is why it runs after it.

use std::f32::consts::{PI, TAU};

use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;

use crate::grid::{Grid, MAX_ROUGH};

// --- Feel knobs: the tunables the user adjusts to taste. ---

/// Target fraction, in [0, 1], of the eligible dry steppe (non-water cells clear of
/// the riparian fringe) that ends up as rough terrain. A real coverage proportion,
/// not an origin count: the number of strokes is derived by dividing this target
/// area by the average area one stroke paints, so the knob means what it says —
/// `0.05` ≈ "about 5% of the dry steppe is rough". Actual coverage runs a touch
/// under target, since strokes overlap and get trimmed near water.
const ROUGH_COVERAGE: f32 = 0.05;
/// Best-candidate sample count K: each origin is the farthest of K random draws.
const ROUGH_SAMPLES: usize = 12;
/// Minimum dryness (1 - water_prox) for a cell to carry rough terrain. Gates both
/// the candidate origins and every stamped cell, so strokes stay well clear of
/// water — origins on the dry steppe, and stroke bodies trimmed off the riparian
/// fringe rather than reaching toward the bank.
const ROUGH_DRYNESS_MIN: f32 = 0.8;
/// Stroke length range, in cells.
const ROUGH_LEN_MIN: f32 = 4.0;
const ROUGH_LEN_MAX: f32 = 10.0;
/// Maximum lateral amplitude of the stroke's bow, in cells.
const ROUGH_AMP: f32 = 2.5;
/// Disk radius stamped around each sampled point along the curve.
const ROUGH_THICKNESS: i32 = 1;
/// Angle jitter around horizontal, in radians (modest, so strokes read horizontal).
const ROUGH_ANGLE_JITTER: f32 = 0.35;

/// Mitchell's best-candidate over `candidates`: pick `count` cells spread with
/// blue-noise spacing. The first origin is a single random candidate; each next
/// origin is the candidate, among `samples` random draws, whose nearest
/// already-placed origin is farthest (Euclidean on `(col, row)`, `col = i % width`).
/// Pure — the placement contract is pinned by tests, not read off the screen.
fn place_origins(
    candidates: &[usize],
    count: usize,
    samples: usize,
    width: usize,
    rng: &mut StdRng,
) -> Vec<usize> {
    if candidates.is_empty() || count == 0 {
        return Vec::new();
    }
    let coords = |i: usize| ((i % width) as f32, (i / width) as f32);

    // Nearest squared distance from `cand` to any already-chosen origin.
    let repels = |chosen: &[usize], cand: usize| -> f32 {
        let (cx, cy) = coords(cand);
        chosen
            .iter()
            .map(|&p| {
                let (px, py) = coords(p);
                let (dx, dy) = (cx - px, cy - py);
                dx * dx + dy * dy
            })
            .fold(f32::INFINITY, f32::min)
    };

    let mut chosen: Vec<usize> = Vec::with_capacity(count.min(candidates.len()));
    chosen.push(candidates[rng.random_range(0..candidates.len())]);

    while chosen.len() < count.min(candidates.len()) {
        let mut best = candidates[0];
        let mut best_dist = -1.0f32;
        for _ in 0..samples.max(1) {
            let cand = candidates[rng.random_range(0..candidates.len())];
            let min_dist = repels(&chosen, cand);
            if min_dist > best_dist {
                best_dist = min_dist;
                best = cand;
            }
        }
        chosen.push(best);
    }
    chosen
}

/// Rasterize one parametric stroke to the set of cells it covers. Walking `t`
/// over `[0, length]`, the base point runs from the origin along `angle`; a
/// lateral offset `amplitude · sin(2π·periods·t/length)` along the perpendicular
/// bows the line. `periods ≈ 0.5` gives a single bow (a crescent); `periods ≈ 1.0`
/// gives an S (a tilde). A disk of radius `thickness` is stamped around each
/// sampled point. Returns unique in-bounds cell indices (`row·width + col`);
/// out-of-bounds samples are skipped.
fn stroke_cells(
    x0: f32,
    y0: f32,
    length: f32,
    angle: f32,
    amplitude: f32,
    periods: f32,
    thickness: i32,
    width: usize,
    height: usize,
) -> Vec<usize> {
    let (ca, sa) = (angle.cos(), angle.sin());
    // Perpendicular direction the lateral offset is applied along.
    let (px, py) = (-sa, ca);

    let mut cells: Vec<usize> = Vec::new();
    // Sample roughly one point per cell of length, plus the endpoints.
    let steps = (length.ceil() as i32).max(1);
    for s in 0..=steps {
        let t = length * s as f32 / steps as f32;
        let lateral = amplitude * (TAU * periods * t / length.max(f32::EPSILON)).sin();
        let bx = x0 + t * ca + lateral * px;
        let by = y0 + t * sa + lateral * py;

        // Stamp a small disk around the sampled point.
        for dy in -thickness..=thickness {
            for dx in -thickness..=thickness {
                if dx * dx + dy * dy > thickness * thickness {
                    continue;
                }
                let col = bx.round() as i32 + dx;
                let row = by.round() as i32 + dy;
                if col < 0 || row < 0 || col >= width as i32 || row >= height as i32 {
                    continue;
                }
                let idx = row as usize * width + col as usize;
                if !cells.contains(&idx) {
                    cells.push(idx);
                }
            }
        }
    }
    cells
}

/// Author rough-terrain presence as discrete placed strokes. Candidate origins
/// are dry, non-water cells; `ROUGH_COUNT` of them are spaced by best-candidate.
/// Each origin grows one stroke with per-mark randomized length, amplitude,
/// angle, and curve count, rasterized and stamped at `MAX_ROUGH` on its non-water
/// cells. Reads `water`/`water_prox` straight off the grid, which the water layer
/// has already filled by the time this layer runs.
///
/// `rough_seed` is decorrelated from the soil, river, and shrub seeds so the
/// strokes fall independently of the other layers; the orchestrator derives it
/// from the master world seed.
pub(super) fn seed_rough(grid: &mut Grid, rough_seed: u64) {
    let mut rng = StdRng::seed_from_u64(rough_seed);
    let (width, height) = (grid.width(), grid.height());

    // Candidate origins: dry steppe, never on water.
    let candidates: Vec<usize> = (0..grid.len())
        .filter(|&i| grid.water(i) == 0.0 && 1.0 - grid.water_prox(i) >= ROUGH_DRYNESS_MIN)
        .collect();

    // Derive the stroke count from a target *coverage* of the eligible space, not a
    // count of origins: one stroke paints ~(mean length × ribbon width) cells, so
    // dividing the target rough area by that footprint makes ROUGH_COVERAGE an
    // honest [0, 1] proportion of the dry steppe rather than an origin density.
    let avg_len = 0.5 * (ROUGH_LEN_MIN + ROUGH_LEN_MAX);
    let stroke_footprint = avg_len * (2.0 * ROUGH_THICKNESS as f32 + 1.0);
    let target_area = candidates.len() as f32 * ROUGH_COVERAGE;
    let count = (target_area / stroke_footprint).round() as usize;
    let origins = place_origins(&candidates, count, ROUGH_SAMPLES, width, &mut rng);

    for origin in origins {
        let (col, row) = grid.col_row(origin);
        let (x0, y0) = (col as f32, row as f32);

        // Per-stroke params drawn from the rng.
        let base_angle = if rng.random::<bool>() { 0.0 } else { PI };
        let angle = base_angle + rng.random_range(-ROUGH_ANGLE_JITTER..ROUGH_ANGLE_JITTER);
        let length = rng.random_range(ROUGH_LEN_MIN..ROUGH_LEN_MAX);
        let amplitude = rng.random_range(0.0..ROUGH_AMP);
        // Crescent (half bow) or tilde (full S).
        let periods = if rng.random::<bool>() { 0.5 } else { 1.0 };

        let cells = stroke_cells(
            x0, y0, length, angle, amplitude, periods, ROUGH_THICKNESS, width, height,
        );
        for i in cells {
            // Trim the stroke off the riparian fringe: a stamped cell must be dry
            // and unwatered, so bodies never reach toward the bank.
            if grid.water(i) == 0.0 && 1.0 - grid.water_prox(i) >= ROUGH_DRYNESS_MIN {
                grid.set_rough(i, MAX_ROUGH);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strokes_are_spaced() {
        // A dense grid of candidate cells on a 64-wide field. Best-candidate
        // origins keep a min pairwise distance above a floor random placement
        // would routinely violate.
        let width = 64usize;
        let height = 64usize;
        let candidates: Vec<usize> = (0..width * height).collect();
        let mut rng = StdRng::seed_from_u64(42);
        let count = 8;
        let origins = place_origins(&candidates, count, 16, width, &mut rng);

        assert_eq!(origins.len(), count);

        let coords = |i: usize| ((i % width) as f32, (i / width) as f32);
        let mut min_pair = f32::INFINITY;
        for a in 0..origins.len() {
            for b in (a + 1)..origins.len() {
                let (ax, ay) = coords(origins[a]);
                let (bx, by) = coords(origins[b]);
                let d = ((ax - bx).powi(2) + (ay - by).powi(2)).sqrt();
                min_pair = min_pair.min(d);
            }
        }
        assert!(
            min_pair > 8.0,
            "strokes should be blue-noise spaced; min pairwise distance was {min_pair}"
        );
    }

    #[test]
    fn stroke_is_horizontalish() {
        // An angle≈0 stroke spans more in x than in y.
        let cells = stroke_cells(20.0, 20.0, 10.0, 0.0, 1.0, 0.5, 1, 64, 64);
        let cols: Vec<i32> = cells.iter().map(|&i| (i % 64) as i32).collect();
        let rows: Vec<i32> = cells.iter().map(|&i| (i / 64) as i32).collect();
        let span_x = cols.iter().max().unwrap() - cols.iter().min().unwrap();
        let span_y = rows.iter().max().unwrap() - rows.iter().min().unwrap();
        assert!(span_x > span_y, "horizontal stroke: span_x={span_x}, span_y={span_y}");
    }

    #[test]
    fn stroke_amplitude_controls_curve() {
        // amplitude>0 yields vertical deviation; amplitude==0 yields a straight line.
        let curved = stroke_cells(20.0, 20.0, 10.0, 0.0, 2.5, 0.5, 0, 64, 64);
        let rows: Vec<i32> = curved.iter().map(|&i| (i / 64) as i32).collect();
        let dev = rows.iter().max().unwrap() - rows.iter().min().unwrap();
        assert!(dev > 0, "amplitude>0 should bow the stroke off its axis (dev={dev})");

        let straight = stroke_cells(20.0, 20.0, 10.0, 0.0, 0.0, 0.5, 0, 64, 64);
        let rows: Vec<i32> = straight.iter().map(|&i| (i / 64) as i32).collect();
        let dev = rows.iter().max().unwrap() - rows.iter().min().unwrap();
        assert_eq!(dev, 0, "amplitude==0 should stay on its axis (dev={dev})");
    }

    #[test]
    fn stroke_cells_in_bounds() {
        // Every returned index is < width·height, even when the stroke runs off
        // the edge of the field.
        let width = 32usize;
        let height = 24usize;
        let cells = stroke_cells(30.0, 22.0, 12.0, 0.0, 3.0, 1.0, 2, width, height);
        for i in cells {
            assert!(i < width * height, "index {i} out of bounds for {width}x{height}");
        }
    }
}
