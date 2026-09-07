//! Pure scalar metrics over centroid time series for the three verified herd behaviours.
//! Headless runner lives in `sim_harness` (`run_behavior`).

/// Generic reductions over one metric column from a `MetricLog` (or any `&[f32]` series).
/// These are the analysis half of the instrumentation: a test reads a column and asserts a shape.
pub mod series {
    pub fn final_value(s: &[f32]) -> f32 {
        s.last().copied().unwrap_or(0.0)
    }

    pub fn min(s: &[f32]) -> f32 {
        s.iter().copied().fold(f32::INFINITY, f32::min)
    }

    pub fn max(s: &[f32]) -> f32 {
        s.iter().copied().fold(f32::NEG_INFINITY, f32::max)
    }

    pub fn mean(s: &[f32]) -> f32 {
        if s.is_empty() { 0.0 } else { s.iter().sum::<f32>() / s.len() as f32 }
    }

    /// Least-squares slope of the series against its own index — the per-tick trend.
    /// Positive ⇒ rising over time. Zero for fewer than two points.
    pub fn slope(s: &[f32]) -> f32 {
        let n = s.len();
        if n < 2 {
            return 0.0;
        }
        let n_f = n as f32;
        let mean_x = (n_f - 1.0) / 2.0;
        let mean_y = mean(s);
        let (mut cov, mut var) = (0.0_f32, 0.0_f32);
        for (i, &y) in s.iter().enumerate() {
            let dx = i as f32 - mean_x;
            cov += dx * (y - mean_y);
            var += dx * dx;
        }
        if var <= f32::EPSILON { 0.0 } else { cov / var }
    }

    /// First index whose value is ≥ `threshold` — e.g. the tick a metric first crosses a bar.
    pub fn first_reaching(s: &[f32], threshold: f32) -> Option<usize> {
        s.iter().position(|&v| v >= threshold)
    }

    /// Population standard deviation — the amplitude of a metric's swing. A settled signal
    /// sits near 0; a herd flip-flopping between all-grazing and all-moving reads high.
    pub fn std(s: &[f32]) -> f32 {
        if s.is_empty() {
            return 0.0;
        }
        let m = mean(s);
        let var = s.iter().map(|v| (v - m) * (v - m)).sum::<f32>() / s.len() as f32;
        var.sqrt()
    }

    /// How many times the series crosses its own mean — the cadence of an oscillation.
    /// A steady signal ≈ 0; a regular high count is the "settle all-at-once, then all move"
    /// phase-locked flip-flop. Exact-mean samples don't reset the side, so a plateau at the
    /// mean isn't miscounted as a crossing.
    pub fn mean_crossings(s: &[f32]) -> usize {
        let m = mean(s);
        let mut count = 0;
        let mut prev_sign = 0i8;
        for &v in s {
            let sign = if v > m { 1 } else if v < m { -1 } else { 0 };
            if sign != 0 {
                if prev_sign != 0 && sign != prev_sign {
                    count += 1;
                }
                prev_sign = sign;
            }
        }
        count
    }
}

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
    fn series_reductions() {
        let s = [1.0, 3.0, 2.0, 6.0];
        assert_eq!(series::final_value(&s), 6.0);
        assert_eq!(series::min(&s), 1.0);
        assert_eq!(series::max(&s), 6.0);
        assert_eq!(series::mean(&s), 3.0);
        assert_eq!(series::first_reaching(&s, 5.0), Some(3));
        assert_eq!(series::first_reaching(&s, 99.0), None);
    }

    #[test]
    fn std_is_zero_when_flat_and_grows_with_swing() {
        assert_eq!(series::std(&[0.5, 0.5, 0.5, 0.5]), 0.0, "settled signal → zero spread");
        assert_eq!(series::std(&[]), 0.0, "empty → zero");
        // A herd flip-flopping 0↔1 sits at the extreme: mean 0.5, every point 0.5 off it.
        assert!((series::std(&[0.0, 1.0, 0.0, 1.0]) - 0.5).abs() < 1e-6, "0↔1 swing → std 0.5");
    }

    #[test]
    fn mean_crossings_counts_oscillation_not_plateau() {
        assert_eq!(series::mean_crossings(&[0.5, 0.5, 0.5]), 0, "flat → no crossings");
        // mean is 0.5; the four sign flips of (x-mean) are the phase-locked flip-flop.
        assert_eq!(series::mean_crossings(&[0.0, 1.0, 0.0, 1.0, 0.0]), 4, "0↔1↔0↔1↔0 → 4 crossings");
        // a run sitting exactly at the mean must not count as crossings
        assert_eq!(series::mean_crossings(&[1.0, 1.0, 1.0, 1.0]), 0, "plateau at mean → none");
        assert_eq!(series::mean_crossings(&[0.0, 2.0]), 1, "one rise past the mean → one crossing");
    }

    #[test]
    fn slope_signs_match_trend() {
        assert!(series::slope(&[0.0, 1.0, 2.0, 3.0]) > 0.0, "rising series → positive slope");
        assert!(series::slope(&[3.0, 2.0, 1.0, 0.0]) < 0.0, "falling series → negative slope");
        assert_eq!(series::slope(&[5.0, 5.0, 5.0]), 0.0, "flat series → zero slope");
        assert_eq!(series::slope(&[1.0]), 0.0, "single point → zero slope");
    }

    #[test]
    fn slope_recovers_a_known_rate() {
        // y = 2·index + 10 → slope 2.
        let s: Vec<f32> = (0..10).map(|i| 2.0 * i as f32 + 10.0).collect();
        assert!((series::slope(&s) - 2.0).abs() < 1e-4, "slope {} should be ~2", series::slope(&s));
    }

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
