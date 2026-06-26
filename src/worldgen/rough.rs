//! Rough-terrain layer: crescent/tilde strokes at Mitchell best-candidate origins on dry steppe.
//! Derive→Distribute approach (doc01.04, doc02.02) — placed discrete features, not thresholded noise.

use std::f32::consts::{PI, TAU};

use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;

use crate::grid::{Grid, MAX_ROUGH};

/// Fraction of eligible dry steppe to cover; stroke count is derived (not literal).
const ROUGH_COVERAGE: f32 = 0.05;
const ROUGH_SAMPLES: usize = 12;
/// Gates both origin candidates AND stamped cells, so stroke bodies stay off the riparian fringe.
const ROUGH_DRYNESS_MIN: f32 = 0.8;
const ROUGH_LEN_MIN: f32 = 4.0;
const ROUGH_LEN_MAX: f32 = 10.0;
const ROUGH_AMP: f32 = 2.5;
const ROUGH_THICKNESS: i32 = 1;
/// Modest jitter so strokes remain roughly horizontal.
const ROUGH_ANGLE_JITTER: f32 = 0.35;

/// Mitchell's best-candidate: picks `count` blue-noise-spaced origins from `candidates`.
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

/// Rasterize a parametric bow stroke; periods≈0.5 → crescent, ≈1.0 → tilde.
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
    let (px, py) = (-sa, ca); // perpendicular for lateral bow

    let mut cells: Vec<usize> = Vec::new();
    let steps = (length.ceil() as i32).max(1);
    for s in 0..=steps {
        let t = length * s as f32 / steps as f32;
        let lateral = amplitude * (TAU * periods * t / length.max(f32::EPSILON)).sin();
        let bx = x0 + t * ca + lateral * px;
        let by = y0 + t * sa + lateral * py;

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

pub(super) fn seed_rough(grid: &mut Grid, rough_seed: u64) {
    let mut rng = StdRng::seed_from_u64(rough_seed);
    let (width, height) = (grid.width(), grid.height());

    let candidates: Vec<usize> = (0..grid.len())
        .filter(|&i| grid.water(i) == 0.0 && 1.0 - grid.water_prox(i) >= ROUGH_DRYNESS_MIN)
        .collect();

    let avg_len = 0.5 * (ROUGH_LEN_MIN + ROUGH_LEN_MAX);
    let stroke_footprint = avg_len * (2.0 * ROUGH_THICKNESS as f32 + 1.0);
    let target_area = candidates.len() as f32 * ROUGH_COVERAGE;
    let count = (target_area / stroke_footprint).round() as usize;
    let origins = place_origins(&candidates, count, ROUGH_SAMPLES, width, &mut rng);

    for origin in origins {
        let (col, row) = grid.col_row(origin);
        let (x0, y0) = (col as f32, row as f32);

        let base_angle = if rng.random::<bool>() { 0.0 } else { PI };
        let angle = base_angle + rng.random_range(-ROUGH_ANGLE_JITTER..ROUGH_ANGLE_JITTER);
        let length = rng.random_range(ROUGH_LEN_MIN..ROUGH_LEN_MAX);
        let amplitude = rng.random_range(0.0..ROUGH_AMP);
        let periods = if rng.random::<bool>() { 0.5 } else { 1.0 };

        let cells = stroke_cells(
            x0, y0, length, angle, amplitude, periods, ROUGH_THICKNESS, width, height,
        );
        for i in cells {
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
        let cells = stroke_cells(20.0, 20.0, 10.0, 0.0, 1.0, 0.5, 1, 64, 64);
        let cols: Vec<i32> = cells.iter().map(|&i| (i % 64) as i32).collect();
        let rows: Vec<i32> = cells.iter().map(|&i| (i / 64) as i32).collect();
        let span_x = cols.iter().max().unwrap() - cols.iter().min().unwrap();
        let span_y = rows.iter().max().unwrap() - rows.iter().min().unwrap();
        assert!(span_x > span_y, "horizontal stroke: span_x={span_x}, span_y={span_y}");
    }

    #[test]
    fn stroke_amplitude_controls_curve() {
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
        let width = 32usize;
        let height = 24usize;
        let cells = stroke_cells(30.0, 22.0, 12.0, 0.0, 3.0, 1.0, 2, width, height);
        for i in cells {
            assert!(i < width * height, "index {i} out of bounds for {width}x{height}");
        }
    }
}
