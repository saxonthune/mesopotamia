//! Service layer: pure, domain-agnostic algorithms over a rectangular lattice of
//! scalar cells. Nothing here knows about grass, water, or Bevy — callers pass a
//! flat `&[f32]` plus the lattice `width`/`height` and get a new `Vec<f32>` back.
//! Plugins compose these into domain rules.

use rand::Rng;

const NEIGHBORS: [(isize, isize); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];

/// 4-neighbour step with bounds, mirroring the lattice adjacency the rest of the
/// sim uses. `None` off the edge, so callers simply skip the missing neighbour.
fn step(index: usize, dx: isize, dy: isize, width: usize, height: usize) -> Option<usize> {
    let col = (index % width) as isize + dx;
    let row = (index / width) as isize + dy;
    if col < 0 || col >= width as isize || row < 0 || row >= height as isize {
        None
    } else {
        Some(row as usize * width + col as usize)
    }
}

/// Mean of a cell's 4-neighbours (edge cells average only the neighbours present).
fn neighbor_mean(field: &[f32], index: usize, width: usize, height: usize) -> f32 {
    let mut sum = 0.0;
    let mut count = 0.0;
    for (dx, dy) in NEIGHBORS {
        if let Some(n) = step(index, dx, dy, width, height) {
            sum += field[n];
            count += 1.0;
        }
    }
    if count > 0.0 { sum / count } else { 0.0 }
}

/// Box-blur `passes` times: each cell becomes the average of itself and its
/// 4-neighbours, double-buffered so a pass reads the previous state, never its
/// own half-written output. Turns white noise into low-frequency value noise;
/// more passes widen the features.
pub fn smooth(field: &[f32], width: usize, height: usize, passes: usize) -> Vec<f32> {
    let mut cur = field.to_vec();
    for _ in 0..passes {
        let mut next = cur.clone();
        for i in 0..cur.len() {
            let mut sum = cur[i];
            let mut count = 1.0;
            for (dx, dy) in NEIGHBORS {
                if let Some(n) = step(i, dx, dy, width, height) {
                    sum += cur[n];
                    count += 1.0;
                }
            }
            next[i] = sum / count;
        }
        cur = next;
    }
    cur
}

/// White noise smoothed into spatially-correlated value noise. Raw output sits in
/// [0, 1] but bunches toward 0.5 as `passes` rises — `normalize` to restore contrast.
pub fn value_noise(width: usize, height: usize, passes: usize, rng: &mut impl Rng) -> Vec<f32> {
    let white: Vec<f32> = (0..width * height).map(|_| rng.random::<f32>()).collect();
    smooth(&white, width, height, passes)
}

/// Rescale so the field's min→0 and max→1. A flat field maps to all-zeros.
pub fn normalize(field: &[f32]) -> Vec<f32> {
    let min = field.iter().copied().fold(f32::INFINITY, f32::min);
    let max = field.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let range = max - min;
    if range <= f32::EPSILON {
        return vec![0.0; field.len()];
    }
    field.iter().map(|v| (v - min) / range).collect()
}

/// One reaction-diffusion growth step. Each cell grows toward its `cap` at a rate
/// seeded by how much its neighbours already hold, so bare ground is recolonised
/// from its green edges inward while the interior of a large hole lags. `floor` is
/// the intrinsic growth fraction with no green neighbour (lets an isolated cell
/// seed slowly); `spread` scales the extra growth driven by neighbouring cover.
/// Zero-capacity cells (e.g. water) stay empty and never propagate. Double-buffered
/// — reads the snapshot, returns the next field — so it is order-independent.
pub fn spread_grow(
    field: &[f32],
    cap: &[f32],
    width: usize,
    height: usize,
    floor: f32,
    spread: f32,
) -> Vec<f32> {
    let mut next = field.to_vec();
    for i in 0..field.len() {
        let c = cap[i];
        if c <= 0.0 {
            next[i] = 0.0;
            continue;
        }
        let cover = neighbor_mean(field, i, width, height);
        let rate = floor + spread * cover;
        next[i] = (field[i] + rate * (c - field[i])).clamp(0.0, c);
    }
    next
}

#[cfg(test)]
mod tests {
    use super::*;

    // A field with no variation has nothing to blur — every cell stays put.
    #[test]
    fn smooth_preserves_a_flat_field() {
        let out = smooth(&vec![0.5; 16], 4, 4, 3);
        assert!(out.iter().all(|v| (v - 0.5).abs() < 1e-6));
    }

    #[test]
    fn normalize_stretches_to_unit_range() {
        assert_eq!(normalize(&[2.0, 3.0, 4.0]), vec![0.0, 0.5, 1.0]);
    }

    #[test]
    fn normalize_flat_field_is_zero() {
        assert_eq!(normalize(&[7.0, 7.0]), vec![0.0, 0.0]);
    }

    // The core of #3: grass colonises a bare cell from a full neighbour.
    #[test]
    fn grass_spreads_into_a_bare_neighbour() {
        let next = spread_grow(&[1.0, 0.0], &[1.0, 1.0], 2, 1, 0.0, 0.5);
        assert!(next[1] > 0.0, "bare cell colonised from its neighbour");
        assert!(next[1] <= 1.0);
    }

    // Water (zero capacity) never holds grass and blocks propagation.
    #[test]
    fn zero_capacity_stays_empty() {
        let next = spread_grow(&[1.0, 0.5], &[0.0, 1.0], 2, 1, 0.5, 0.5);
        assert_eq!(next[0], 0.0);
    }

    // With no intrinsic floor and no green anywhere, nothing can start.
    #[test]
    fn bare_field_with_no_floor_stays_bare() {
        let next = spread_grow(&[0.0; 4], &[1.0; 4], 2, 2, 0.0, 0.9);
        assert!(next.iter().all(|&v| v == 0.0));
    }
}
