//! Pure scalar metrics over centroid time series for the three verified herd behaviours.
//! Headless runner lives in `sim_harness` (`run_behavior`).

/// Net displacement along the east axis; positive ⇒ moved east.
pub fn eastward_drift(cols: &[f32]) -> f32 {
    match (cols.first(), cols.last()) {
        (Some(first), Some(last)) => last - first,
        _ => 0.0,
    }
}

pub fn path_length(cols: &[f32], rows: &[f32]) -> f32 {
    cols.windows(2)
        .zip(rows.windows(2))
        .map(|(c, r)| ((c[1] - c[0]).powi(2) + (r[1] - r[0]).powi(2)).sqrt())
        .sum()
}

/// Net displacement ÷ path length; ~1 for a straight march, ~0 for a closed loop.
pub fn straightness(cols: &[f32], rows: &[f32]) -> f32 {
    let path = path_length(cols, rows);
    if path < 1e-6 {
        return 1.0; // held in place — not circling
    }
    let (dc, dr) = (eastward_drift(cols), eastward_drift(rows));
    (dc * dc + dr * dr).sqrt() / path
}

pub fn is_circling(cols: &[f32], rows: &[f32], min_path: f32, min_straight: f32) -> bool {
    path_length(cols, rows) > min_path && straightness(cols, rows) < min_straight
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drift_is_last_minus_first() {
        assert_eq!(eastward_drift(&[2.0, 5.0, 9.0]), 7.0);
        assert_eq!(eastward_drift(&[]), 0.0);
    }

    #[test]
    fn path_length_grows_with_detours() {
        let straight = path_length(&[0.0, 1.0, 2.0], &[0.0, 0.0, 0.0]);
        let detour = path_length(&[0.0, 1.0, 2.0], &[0.0, 3.0, 0.0]);
        assert!(detour > straight, "a detour lengthens the path: {detour} > {straight}");
    }

    #[test]
    fn straight_line_is_fully_straight() {
        let s = straightness(&[0.0, 1.0, 2.0, 3.0], &[0.0, 0.0, 0.0, 0.0]);
        assert!((s - 1.0).abs() < 1e-5, "straight march → 1, got {s}");
    }

    #[test]
    fn closed_loop_reads_as_circling() {
        let cols = [0.0, 1.0, 1.0, 0.0, 0.0];
        let rows = [0.0, 0.0, 1.0, 1.0, 0.0];
        let s = straightness(&cols, &rows);
        assert!(s < 1e-5, "a loop nets nothing → straightness ~0, got {s}");
        assert!(is_circling(&cols, &rows, 1.0, 0.5), "long path, zero net → circling");
    }

    #[test]
    fn held_herd_is_not_circling() {
        let cols = [4.0, 4.0, 4.01, 4.0, 4.0];
        let rows = [3.0, 3.01, 3.0, 3.0, 3.0];
        assert!(!is_circling(&cols, &rows, 1.0, 0.5), "tiny path ⇒ not circling");
    }
}
