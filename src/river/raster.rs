//! Stamping carved centerlines into the grid's water field.

use crate::grid::Grid;
use super::spec::RiverSpec;
use super::cost::lerp;

/// Max-merge so overlapping channels keep the deeper value.
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

/// `profile[t]` in [0,1]: 0 = riffle (shallow+wide), 1 = pool (deep+narrow).
pub(super) fn stamp_main_channel(
    grid: &mut Grid,
    centerline: &[usize],
    profile: &[f32],
    spec: &RiverSpec,
) {
    for (t, &center) in centerline.iter().enumerate() {
        let p = profile[t];
        let depth = lerp(spec.riffle_depth, 1.0, p);
        let radius = lerp(spec.riffle_radius as f32, spec.pool_radius as f32, p).round() as isize;
        rasterize(grid, &[center], radius, spec.core, depth);
    }
}

pub(super) fn tag_shallows_as_fords(grid: &mut Grid, threshold: f32) {
    for i in 0..grid.len() {
        let w = grid.water(i);
        grid.set_ford(i, w > 0.0 && w <= threshold);
    }
}
