//! Pure spatial metrics for reading herd shape out of a simulation run.
//!
//! These are the scalars that distinguish the three things a herd can look like
//! when you can't watch it render: a **clump** (collapsing to a point, then
//! starving in place), a **particle cloud** (bounded spread, no net travel), and
//! a **rolling wave** (bounded spread that advances). Each function is a plain
//! `fn` of cell indices and the grid width — no Bevy, no ECS — so the behaviour
//! they measure is testable in isolation. `sim_harness::diagnose` samples them
//! per tick into a `RunTrace`; the integration tests assert how they must move.

/// Mean `(col, row)` of a set of cells on a `width`-wide grid. The herd's centre
/// of mass; its column over time is the migration signal. Empty set → origin.
pub fn centroid(cells: &[usize], width: usize) -> (f32, f32) {
    if cells.is_empty() {
        return (0.0, 0.0);
    }
    let (mut sx, mut sy) = (0.0_f32, 0.0_f32);
    for &c in cells {
        sx += (c % width) as f32;
        sy += (c / width) as f32;
    }
    let n = cells.len() as f32;
    (sx / n, sy / n)
}

/// Radius of gyration — RMS distance of the cells from their centroid, in cells.
/// This is the herd-spread metric: it collapses toward 0 as a herd clumps to a
/// point and grows as it disperses, independent of where the herd is or how many
/// elk there are. A clumping bug shows up here as a curve that decays to ~0.
pub fn radius_of_gyration(cells: &[usize], width: usize) -> f32 {
    if cells.is_empty() {
        return 0.0;
    }
    let (cx, cy) = centroid(cells, width);
    let mut sum_sq = 0.0_f32;
    for &c in cells {
        let dx = (c % width) as f32 - cx;
        let dy = (c / width) as f32 - cy;
        sum_sq += dx * dx + dy * dy;
    }
    (sum_sq / cells.len() as f32).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── centroid ────────────────────────────────────────────────────────────

    #[test]
    fn centroid_of_a_single_cell_is_that_cell() {
        // cell 13 on a 10-wide grid → (col 3, row 1).
        assert_eq!(centroid(&[13], 10), (3.0, 1.0));
    }

    #[test]
    fn centroid_is_the_mean_position() {
        // cells (0,0) and (2,0) → mean col 1, row 0.
        assert_eq!(centroid(&[0, 2], 10), (1.0, 0.0));
    }

    #[test]
    fn centroid_of_empty_is_origin() {
        assert_eq!(centroid(&[], 10), (0.0, 0.0));
    }

    // ── radius_of_gyration ────────────────────────────────────────────────────

    // A herd stacked on one cell has zero spread — the clump signature.
    #[test]
    fn gyration_of_coincident_cells_is_zero() {
        assert_eq!(radius_of_gyration(&[7, 7, 7, 7], 10), 0.0);
    }

    // Metamorphic: spreading the same elk out can only raise the radius, never
    // lower it. The input change that would break this — moving a cell toward the
    // centroid — is exactly what a clumping herd does, so the relation has teeth.
    #[test]
    fn gyration_rises_as_the_herd_spreads() {
        let tight = radius_of_gyration(&[44, 45, 54, 55], 10); // 2×2 block
        let loose = radius_of_gyration(&[0, 9, 90, 99], 10); // grid corners
        assert!(loose > tight, "dispersed herd must have larger gyration");
    }

    // Translation invariance: the metric measures shape, not location, so shifting
    // every cell by the same offset leaves the radius unchanged. This is what lets
    // a rolling herd advance (centroid moves) without perturbing the spread signal.
    #[test]
    fn gyration_is_translation_invariant() {
        let here = radius_of_gyration(&[0, 1, 10, 11], 10); // 2×2 at origin
        let shifted = radius_of_gyration(&[5, 6, 15, 16], 10); // same block, +5 cols
        assert!((here - shifted).abs() < 1e-6);
    }
}
