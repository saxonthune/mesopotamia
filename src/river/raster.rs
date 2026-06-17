//! Stamping carved centerlines into the grid's water field.

use crate::grid::Grid;
use super::spec::RiverSpec;
use super::cost::lerp;

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

/// Stamp a main channel with varying depth and width driven by a riffle/pool `profile`.
/// `profile[t]` in [0,1]: 0 = full riffle (shallow + wide), 1 = full pool (deep + narrow).
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

/// Tag every water cell at or below `threshold` as a fordable crossing.
pub(super) fn tag_shallows_as_fords(grid: &mut Grid, threshold: f32) {
    for i in 0..grid.len() {
        let w = grid.water(i);
        grid.set_ford(i, w > 0.0 && w <= threshold);
    }
}
