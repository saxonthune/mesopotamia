//! Service layer: pure, domain-agnostic algorithms over a rectangular lattice of
//! scalar cells. Nothing here knows about grass, water, or Bevy — callers pass a
//! flat `&[f32]` plus the lattice `width`/`height` and get a new `Vec<f32>` back.
//! Plugins compose these into domain rules.

use std::f32::consts::TAU;

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

/// Single-axis box-blur: like `smooth`, but averages each cell only with its two
/// neighbours along one axis — horizontal when `horizontal`, else vertical.
/// Applied after isotropic smoothing, a few horizontal passes stretch round noise
/// blobs east-west into a lateral grain *without changing the value distribution's
/// quantiles* — so a downstream quantile cut (e.g. shrub coverage) keeps the same
/// fraction of cells; only the features' shape elongates. `passes == 0` is identity.
pub fn smooth_axis(field: &[f32], width: usize, height: usize, passes: usize, horizontal: bool) -> Vec<f32> {
    let (dx, dy) = if horizontal { (1isize, 0isize) } else { (0, 1) };
    let mut cur = field.to_vec();
    for _ in 0..passes {
        let mut next = cur.clone();
        for i in 0..cur.len() {
            let mut sum = cur[i];
            let mut count = 1.0;
            for s in [-1isize, 1] {
                if let Some(n) = step(i, dx * s, dy * s, width, height) {
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

/// Sample `field` at fractional coordinates `(x, y)` by bilinear interpolation,
/// clamping to the lattice edges so out-of-range reads pin to the border cell.
fn bilinear(field: &[f32], width: usize, height: usize, x: f32, y: f32) -> f32 {
    let x = x.clamp(0.0, (width - 1) as f32);
    let y = y.clamp(0.0, (height - 1) as f32);
    let x0 = x.floor() as usize;
    let y0 = y.floor() as usize;
    let x1 = (x0 + 1).min(width - 1);
    let y1 = (y0 + 1).min(height - 1);
    let fx = x - x0 as f32;
    let fy = y - y0 as f32;
    let top = field[y0 * width + x0] * (1.0 - fx) + field[y0 * width + x1] * fx;
    let bot = field[y1 * width + x0] * (1.0 - fx) + field[y1 * width + x1] * fx;
    top * (1.0 - fy) + bot * fy
}

/// Domain-warp a field: resample `field` at each cell's coordinates displaced by
/// the two warp fields `wx`/`wy` (each expected in roughly [-1, 1]) scaled by
/// `amp` cells, bilinearly. A straight feature in the source bends into a sinuous
/// one whose wavelength is the warp fields' wavelength — the "flow-like geology"
/// transform (doc02.02, Derive). `amp <= 0` returns the field unchanged.
pub fn domain_warp(
    field: &[f32],
    width: usize,
    height: usize,
    wx: &[f32],
    wy: &[f32],
    amp: f32,
) -> Vec<f32> {
    if amp <= 0.0 {
        return field.to_vec();
    }
    (0..field.len())
        .map(|i| {
            let col = (i % width) as f32 + amp * wx[i];
            let row = (i / width) as f32 + amp * wy[i];
            bilinear(field, width, height, col, row)
        })
        .collect()
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

/// Travelling green-up crest at a column, in [0, 1]. Multiple bands march east as
/// `tick` rises: phase = 2π·(col/wavelength − tick·speed). `speed` is cycles per
/// tick (crest advances `wavelength·speed` cells/tick). At `amp == 0` callers use
/// `base` directly and never call this — but the field itself is well-defined for all
/// inputs. Returns (cos(phase)+1)/2 so the crest is 1 and the trough 0.
pub fn green_wave(col: usize, wavelength: f32, tick: f32, speed: f32) -> f32 {
    let phase = TAU * (col as f32 / wavelength - tick * speed);
    (phase.cos() + 1.0) / 2.0
}

/// Like `spread_grow` but with a per-cell intrinsic `floor` and a per-cell `senesce`
/// rate. Each cell grows toward `cap` at `floor[i] + spread·cover`, then relaxes toward
/// 0 at `senesce[i]` (applied to the post-growth value). `floor.len() == senesce.len()
/// == field.len()`. Double-buffered, order-independent, same as `spread_grow`.
pub fn wave_grow(
    field: &[f32],
    cap: &[f32],
    width: usize,
    height: usize,
    floor: &[f32],
    spread: f32,
    senesce: &[f32],
) -> Vec<f32> {
    let mut next = field.to_vec();
    for i in 0..field.len() {
        let c = cap[i];
        if c <= 0.0 {
            next[i] = 0.0;
            continue;
        }
        let cover = neighbor_mean(field, i, width, height);
        let rate = floor[i] + spread * cover;
        let grown = (field[i] + rate * (c - field[i])).clamp(0.0, c);
        next[i] = grown * (1.0 - senesce[i]);
    }
    next
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
    let n = field.len();
    let floor_vec = vec![floor; n];
    let senesce_vec = vec![0.0f32; n];
    wave_grow(field, cap, width, height, &floor_vec, spread, &senesce_vec)
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

    // Horizontal smoothing spreads a value along its row but never across rows —
    // this is what elongates clumps east-west while leaving north-south alone.
    #[test]
    fn smooth_axis_horizontal_spreads_only_along_the_row() {
        // 3x3, a single spike at the centre. One horizontal pass bleeds it left and
        // right; the cells directly above and below stay untouched.
        let mut f = vec![0.0; 9];
        f[4] = 1.0; // centre (col 1, row 1)
        let out = smooth_axis(&f, 3, 3, 1, true);
        assert!(out[3] > 0.0 && out[5] > 0.0, "row neighbours pick up the spike");
        assert_eq!(out[1], 0.0, "the cell above is unchanged by a horizontal pass");
        assert_eq!(out[7], 0.0, "the cell below is unchanged by a horizontal pass");
    }

    // The coverage-preserving guarantee the shrub layer relies on: smoothing
    // averages, so it can't change how many cells sit above the median — a
    // 50%-quantile cut keeps the same count whether or not the field is stretched.
    #[test]
    fn smooth_axis_preserves_the_median_split() {
        // A vertical step: left half 0, right half 1 on an 8x4 grid → exactly half
        // the cells are >= 0.5. Horizontal smoothing blurs the seam but, by symmetry,
        // the same number of cells stay on each side of 0.5.
        let (w, h) = (8usize, 4usize);
        let f: Vec<f32> = (0..w * h).map(|i| if i % w < w / 2 { 0.0 } else { 1.0 }).collect();
        let before = f.iter().filter(|&&v| v >= 0.5).count();
        let out = smooth_axis(&f, w, h, 3, true);
        let after = out.iter().filter(|&&v| v >= 0.5).count();
        assert_eq!(before, after, "the median split is preserved, so coverage holds");
    }

    // Zero amplitude is a no-op — the warp leaves every cell where it was.
    #[test]
    fn domain_warp_zero_amp_is_identity() {
        let src = vec![0.1, 0.2, 0.3, 0.4];
        let z = vec![0.0; 4];
        assert_eq!(domain_warp(&src, 2, 2, &z, &z, 0.0), src);
    }

    // A constant +1-column displacement shifts a horizontal gradient left by one
    // cell (each cell samples its right neighbour); the right edge clamps.
    #[test]
    fn domain_warp_shifts_by_displacement() {
        // 3x1 gradient: [0, 1, 2]. wx = +1 everywhere, amp = 1 → sample x+1.
        let src = vec![0.0, 1.0, 2.0];
        let wx = vec![1.0, 1.0, 1.0];
        let wy = vec![0.0, 0.0, 0.0];
        let out = domain_warp(&src, 3, 1, &wx, &wy, 1.0);
        assert_eq!(out, vec![1.0, 2.0, 2.0]); // last clamps to the edge cell
    }

    // Bilinear sampling at exact integer coordinates returns the cell verbatim.
    #[test]
    fn bilinear_hits_grid_cells_exactly() {
        let f = vec![0.0, 1.0, 2.0, 3.0];
        assert_eq!(bilinear(&f, 2, 2, 1.0, 1.0), 3.0);
        assert_eq!(bilinear(&f, 2, 2, 0.0, 1.0), 2.0);
        // Midpoint between all four averages them.
        assert_eq!(bilinear(&f, 2, 2, 0.5, 0.5), 1.5);
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

    // The crest is at the column where col = wavelength * tick * speed (mod wavelength).
    // With a single crest fitting in the window, the argmax must move east each step.
    #[test]
    fn green_wave_crest_advances_east() {
        let wavelength = 64.0f32;
        let speed = 0.01f32;
        let width = 64usize;
        let argmax = |tick: f32| {
            (0..width)
                .max_by(|&a, &b| {
                    green_wave(a, wavelength, tick, speed)
                        .partial_cmp(&green_wave(b, wavelength, tick, speed))
                        .unwrap()
                })
                .unwrap()
        };
        // At tick 0 the crest is at col 0; at tick 20 it advances to col ~13.
        let t0 = argmax(0.0);
        let t20 = argmax(20.0);
        assert!(
            t20 > t0,
            "crest must advance east: t0={t0} t20={t20}"
        );
    }

    // green_wave(col, λ, t, s) ≈ green_wave(col + λ, λ, t, s) — one-period shift.
    #[test]
    fn green_wave_periodic_in_col() {
        let wavelength = 50.0f32;
        for col in 0..100usize {
            let v0 = green_wave(col, wavelength, 3.7, 0.005);
            let v1 = green_wave(col + wavelength as usize, wavelength, 3.7, 0.005);
            assert!(
                (v0 - v1).abs() < 1e-5,
                "green_wave must be periodic: col={col} v0={v0} v1={v1}"
            );
        }
    }

    // The crest field must stay in [0, 1] for all inputs.
    #[test]
    fn green_wave_bounded() {
        for col in 0..100usize {
            for t in 0..100u32 {
                let v = green_wave(col, 40.0, t as f32 * 0.7, 0.013);
                assert!(
                    (0.0..=1.0).contains(&v),
                    "green_wave out of [0,1]: col={col} t={t} v={v}"
                );
            }
        }
    }

    // With constant floor and zero senesce, wave_grow must match spread_grow exactly.
    #[test]
    fn wave_grow_identity_matches_spread_grow() {
        let field = vec![0.3, 0.0, 0.8, 0.5];
        let cap = vec![1.0, 1.0, 1.0, 1.0];
        let floor_k = 0.05f32;
        let spread = 0.2f32;

        let expected = spread_grow(&field, &cap, 2, 2, floor_k, spread);
        let floor_vec = vec![floor_k; 4];
        let senesce_vec = vec![0.0f32; 4];
        let actual = wave_grow(&field, &cap, 2, 2, &floor_vec, spread, &senesce_vec);

        for (i, (e, a)) in expected.iter().zip(actual.iter()).enumerate() {
            assert!(
                (e - a).abs() < 1e-6,
                "wave_grow identity failed at cell {i}: expected={e} actual={a}"
            );
        }
    }

    // At equal starting grass, a high-senesce cell must hold less after one step.
    #[test]
    fn wave_grow_senescence_decays_troughs_not_crests() {
        let field = vec![0.5, 0.5];
        let cap = vec![1.0, 1.0];
        let floor = vec![0.0, 0.0];
        let senesce = vec![0.1, 0.0]; // cell 0 high senesce, cell 1 none
        let next = wave_grow(&field, &cap, 2, 1, &floor, 0.0, &senesce);
        assert!(
            next[0] < next[1],
            "high-senesce cell must hold less: next[0]={} next[1]={}",
            next[0], next[1]
        );
    }

    // Zero-capacity (water) cells must stay empty even with a non-zero floor.
    #[test]
    fn wave_grow_water_cells_stay_empty() {
        let field = vec![1.0, 0.5];
        let cap = vec![0.0, 1.0]; // cell 0 is water
        let floor = vec![0.5, 0.5];
        let senesce = vec![0.0, 0.0];
        let next = wave_grow(&field, &cap, 2, 1, &floor, 0.0, &senesce);
        assert_eq!(next[0], 0.0, "water cell (cap=0) must stay empty");
    }
}
