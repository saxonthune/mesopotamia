//! Stamping carved centerlines into the grid's water field, plus the periodic
//! ford bands that cross each main channel.

use crate::grid::Grid;

/// Stamp water values around each centerline cell. `max_depth` scales the level:
/// 1.0 gives a full-depth main channel; lower values produce shallow streams/pools.
/// Uses max-merge so overlapping channels keep the deeper value.
pub(super) fn rasterize(
    grid: &mut Grid,
    centerline: &[usize],
    radius: isize,
    core: isize,
    max_depth: f32,
) {
    for &center in centerline {
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                if let Some(cell) = grid.step(center, dx, dy) {
                    let dist = ((dx * dx + dy * dy) as f32).sqrt();
                    let level = if dist <= core as f32 {
                        max_depth
                    } else {
                        (max_depth * (1.0 - dist / (radius as f32 + 1.0))).max(0.0)
                    };
                    if level > grid.water(cell) {
                        grid.set_water(cell, level);
                    }
                }
            }
        }
    }
}

/// Returns every `spacing`-th element of `centerline` by position (0, spacing, 2*spacing, …).
pub(super) fn ford_indices(centerline: &[usize], spacing: usize) -> Vec<usize> {
    centerline.iter().copied().step_by(spacing).collect()
}

/// Stamp periodic shallow crossing bands (horizontal, perpendicular to flow)
/// along EACH main centerline, marking the cells as fords.
pub(super) fn stamp_fords(
    grid: &mut Grid,
    mains: &[Vec<usize>],
    radius: isize,
    spacing: usize,
    depth: f32,
) {
    for main_cl in mains {
        for &ford_center in &ford_indices(main_cl, spacing) {
            for dx in -radius..=radius {
                if let Some(cell) = grid.step(ford_center, dx, 0) {
                    grid.set_water(cell, depth);
                    grid.set_ford(cell, true);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ford_indices_returns_every_nth_cell() {
        let cl: Vec<usize> = (10..110).collect(); // 100 elements: 10..109
        let fords = ford_indices(&cl, 25);
        assert_eq!(fords, vec![10, 35, 60, 85]);
    }

    #[test]
    fn ford_indices_empty_centerline() {
        assert!(ford_indices(&[], 10).is_empty());
    }

    #[test]
    fn ford_indices_spacing_larger_than_len() {
        let cl = vec![7usize, 8, 9];
        let fords = ford_indices(&cl, 10);
        assert_eq!(fords, vec![7]); // only position 0
    }
}
