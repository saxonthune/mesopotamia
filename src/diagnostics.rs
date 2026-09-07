//! Pure spatial metrics (centroid, radius of gyration) for characterising herd shape.
//! No Bevy, no ECS; called by `sim_harness::diagnose` and asserted in integration tests.

/// Mean (col, row) position. Column over time is the migration signal. Empty set → origin.
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

/// RMS distance of cells from their centroid. Herd-spread metric: zero = clump, high = dispersed.
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

    #[test]
    fn gyration_of_coincident_cells_is_zero() {
        assert_eq!(radius_of_gyration(&[7, 7, 7, 7], 10), 0.0);
    }

    // Metamorphic: spread can only raise the radius; moving toward the centroid (clumping) lowers it.
    #[test]
    fn gyration_rises_as_the_herd_spreads() {
        let tight = radius_of_gyration(&[44, 45, 54, 55], 10); // 2×2 block
        let loose = radius_of_gyration(&[0, 9, 90, 99], 10); // grid corners
        assert!(loose > tight, "dispersed herd must have larger gyration");
    }

    // Translation invariance: measures shape not location; a rolling herd's centroid moves without perturbing spread.
    #[test]
    fn gyration_is_translation_invariant() {
        let here = radius_of_gyration(&[0, 1, 10, 11], 10); // 2×2 at origin
        let shifted = radius_of_gyration(&[5, 6, 15, 16], 10); // same block, +5 cols
        assert!((here - shifted).abs() < 1e-6);
    }
}
