//! Pure metrics for the three herd behaviours we verify — move in a direction,
//! cross a river, don't go in circles. Each is a plain function of a centroid time
//! series (or a final headcount), so the behaviour tests assert on a scalar with a
//! known success threshold instead of eyeballing frames. The headless runner that
//! feeds these lives in `sim_harness` (`run_behavior`); the maps it runs on are the
//! `*_plain` builders there.

/// Net displacement along the migration (east) axis: how far the herd centroid
/// ends from where it began, in cells. Positive ⇒ moved east. The success metric
/// for *move in a direction*.
pub fn eastward_drift(cols: &[f32]) -> f32 {
    match (cols.first(), cols.last()) {
        (Some(first), Some(last)) => last - first,
        _ => 0.0,
    }
}

/// Total distance the centroid actually travelled, summing every tick's step in the
/// (col, row) plane. A herd that loops covers a long path; one that holds covers
/// almost none. Paired with `eastward_drift` it separates the two.
pub fn path_length(cols: &[f32], rows: &[f32]) -> f32 {
    cols.windows(2)
        .zip(rows.windows(2))
        .map(|(c, r)| ((c[1] - c[0]).powi(2) + (r[1] - r[0]).powi(2)).sqrt())
        .sum()
}

/// Net straight-line displacement ÷ path actually travelled, in [0, 1]. ~1 for a
/// straight march, →0 for a closed loop (travels far, nets nothing). The discriminant
/// for *not going in circles*: a circling herd has a long path but low straightness.
pub fn straightness(cols: &[f32], rows: &[f32]) -> f32 {
    let path = path_length(cols, rows);
    if path < 1e-6 {
        return 1.0; // held in place — degenerate but not circling
    }
    let (dc, dr) = (eastward_drift(cols), eastward_drift(rows));
    (dc * dc + dr * dr).sqrt() / path
}

/// Whether the centroid track is a circling/milling pattern: it wandered a real
/// distance (`> min_path`) yet netted little of it (`straightness < min_straight`).
/// A held, chewing herd (tiny path) is *not* circling; a directed march (high
/// straightness) is *not* circling; only long-path-low-net trips here.
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

    // Inserting any extra wandering between endpoints can only lengthen the path.
    #[test]
    fn path_length_grows_with_detours() {
        let straight = path_length(&[0.0, 1.0, 2.0], &[0.0, 0.0, 0.0]);
        let detour = path_length(&[0.0, 1.0, 2.0], &[0.0, 3.0, 0.0]);
        assert!(detour > straight, "a detour lengthens the path: {detour} > {straight}");
    }

    // A straight eastward march nets all of its path → straightness 1.
    #[test]
    fn straight_line_is_fully_straight() {
        let s = straightness(&[0.0, 1.0, 2.0, 3.0], &[0.0, 0.0, 0.0, 0.0]);
        assert!((s - 1.0).abs() < 1e-5, "straight march → 1, got {s}");
    }

    // A closed loop returns to start → net 0 → straightness 0, and is flagged circling.
    #[test]
    fn closed_loop_reads_as_circling() {
        let cols = [0.0, 1.0, 1.0, 0.0, 0.0];
        let rows = [0.0, 0.0, 1.0, 1.0, 0.0];
        let s = straightness(&cols, &rows);
        assert!(s < 1e-5, "a loop nets nothing → straightness ~0, got {s}");
        assert!(is_circling(&cols, &rows, 1.0, 0.5), "long path, zero net → circling");
    }

    // A herd that barely moves is held, not circling, even at low straightness.
    #[test]
    fn held_herd_is_not_circling() {
        let cols = [4.0, 4.0, 4.01, 4.0, 4.0];
        let rows = [3.0, 3.01, 3.0, 3.0, 3.0];
        assert!(!is_circling(&cols, &rows, 1.0, 0.5), "tiny path ⇒ not circling");
    }
}
