//! Procedural grass-tile raster: fills a square RGBA canvas with grass blades
//! whose height tracks a density parameter ρ. This is the generative-art operator
//! for the grass layer — form authored as a *rule* and executed per pixel, not
//! drawn by hand or reduced to a glyph. Pure and seeded so the look is pinned by
//! tests and reproducible per cell.
//!
//! Each blade is built to read as a living *center* (after Alexander's fifteen
//! properties): a **good shape** that tapers from a wide base to a one-pixel tip
//! and **curves** (a quadratic sway that leans near the tip); a base-dark →
//! tip-light **gradient**; and wide **height variation** blade to blade for
//! levels of scale. Blades are few and clearly spaced so the mat reads as grass,
//! not a barcode of identical verticals.
//!
//! Blades are *scattered* across the whole tile rather than rooted on a single
//! floor line, and the canvas carries an `overflow` band above the tile so the
//! tallest blades grow up past the tile's top edge. The render anchors that band
//! over the cell above, so a tuft reads as growing into its upper neighbour — a
//! receding, isometric field rather than a row of grass standing on a baseline.

use rand::Rng;
use rand::SeedableRng;
use rand::rngs::StdRng;

/// Tunables for one grass tile.
pub struct GrassTileParams {
    /// Square tile resolution (width and tile-region height), in pixels.
    pub canvas: usize,
    /// Extra rows above the tile region, px. Tall blades grow up into this band,
    /// which the render overlaps onto the cell above so grass spills into its
    /// upper neighbour. The full raster is `canvas` wide × `canvas + overflow` tall.
    pub overflow: usize,
    /// Number of blades scattered across the tile.
    pub blades: usize,
    /// Blade width at the base, px; it tapers toward 1 px at the tip.
    pub base_width: usize,
    /// Lowest root height above the tile bottom, px — the front of the field.
    pub base_min: usize,
    /// Highest root height above the tile bottom, px — the back of the field.
    /// Roots scatter uniformly in `[base_min, base_max]`, so blades stand all
    /// across the tile rather than on one floor line.
    pub base_max: usize,
    /// Full blade length at ρ = 1 before jitter/depth, px. Decoupled from the
    /// root height so a back-rooted blade can still grow up into the overflow.
    pub blade_len: usize,
    /// How much a back-rooted (higher) blade is shortened for isometric depth,
    /// as a fraction of length across the tile (0 = no depth cue, 1 = back blades
    /// vanish). The receding rows of the field read as smaller.
    pub back_shrink: f32,
    /// Maximum horizontal curl of a blade over its length, px. The sway grows
    /// quadratically so the blade is near-straight at the root and leans at the
    /// tip, like a real blade bending under its own weight.
    pub sway: i32,
    /// Fraction of a blade's full height that can be randomly shaved off, blade
    /// to blade, for levels of scale (0 = all blades full height, 1 = down to 0).
    pub height_jitter: f32,
    /// Blade colour at the base (the darker, shaded green near the ground).
    pub base_color: [u8; 3],
    /// Blade colour at the tip (the lighter, yellower green catching the light).
    pub tip_color: [u8; 3],
}

impl Default for GrassTileParams {
    fn default() -> Self {
        Self {
            canvas: 32,
            overflow: 16,
            blades: 18,
            base_width: 3,
            base_min: 0,
            base_max: 26,
            blade_len: 18,
            back_shrink: 0.3,
            sway: 4,
            height_jitter: 0.45,
            base_color: [40, 95, 30],
            tip_color: [120, 180, 70],
        }
    }
}

/// Linear blend of two RGB colours at `t` in `[0, 1]`.
fn lerp_rgb(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 3] {
    let t = t.clamp(0.0, 1.0);
    let mix = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    [mix(a[0], b[0]), mix(a[1], b[1]), mix(a[2], b[2])]
}

/// Rasterize a grass tile into RGBA8 bytes: `canvas*(canvas+overflow)*4` bytes,
/// row-major with the image's top-left as origin. The bottom `canvas` rows are the
/// tile itself; the top `overflow` rows are the band blades grow up into (the
/// render overlaps it onto the cell above). Blades grow *up* from scattered roots.
/// `rho` in `[0, 1]` is the grass-height fraction; `rho <= 0` yields a fully
/// transparent tile. Deterministic in `seed`.
pub fn rasterize_grass(p: &GrassTileParams, rho: f32, seed: u64) -> Vec<u8> {
    let w = p.canvas; // tile width = tile-region height
    let h_full = p.canvas + p.overflow; // raster height: tile region + overflow band
    let mut buf = vec![0u8; w * h_full * 4]; // starts fully transparent
    let rho = rho.clamp(0.0, 1.0);
    if rho <= 0.0 || w == 0 {
        return buf;
    }
    let mut rng = StdRng::seed_from_u64(seed);
    let root_lo = p.base_min.min(w.saturating_sub(1));
    let root_hi = p.base_max.min(w.saturating_sub(1)).max(root_lo);
    let base_w = p.base_width.max(1) as f32;

    // Paint one pixel, alpha-opaque. `cx`/`y` are blade-space (y up from the tile
    // bottom; y >= canvas is the overflow band above the tile).
    let put = |buf: &mut [u8], cx: i32, y: usize, col: [u8; 3]| {
        if cx < 0 || cx >= w as i32 || y >= h_full {
            return;
        }
        let row = h_full - 1 - y; // flip: image row 0 is the top of the overflow band
        let idx = (row * w + cx as usize) * 4;
        buf[idx] = col[0];
        buf[idx + 1] = col[1];
        buf[idx + 2] = col[2];
        buf[idx + 3] = 255;
    };

    for _ in 0..p.blades {
        // Root x, kept off the edges so a swaying blade still fits.
        let margin = (base_w as i32 / 2).max(1);
        let lo = margin;
        let hi = (w as i32 - 1 - margin).max(lo);
        let x0 = rng.random_range(lo..=hi);
        // Root scattered across the tile height → a field, not a floor line.
        let base = if root_hi > root_lo {
            rng.random_range(root_lo..=root_hi)
        } else {
            root_lo
        };
        // Isometric depth: blades rooted further back (higher) grow shorter.
        let depth = 1.0 - p.back_shrink * (base as f32 / w as f32);
        // Per-blade height factor → levels of scale; ρ scales them all together.
        // Length is the fixed blade reach (not the room to the top), so a
        // back-rooted blade still climbs into the overflow band.
        let hfac = 1.0 - p.height_jitter * rng.random::<f32>();
        let height = ((rho * p.blade_len as f32 * hfac * depth).round() as usize).max(1);
        // Curl direction/strength and a per-blade shade so the mat isn't flat.
        let sway = if p.sway > 0 {
            rng.random_range(-p.sway..=p.sway) as f32
        } else {
            0.0
        };
        let shade = rng.random_range(-12i32..=12);

        for h in 0..height {
            let y = base + h;
            if y >= h_full {
                break;
            }
            let t = h as f32 / height as f32; // 0 at floor, →1 at tip
            // Quadratic curl: straight at the base, leaning at the tip.
            let cx = x0 + (sway * t * t).round() as i32;
            // Taper: full width at the base down to a single pixel at the tip.
            let bw = (base_w * (1.0 - 0.7 * t)).round().max(1.0) as i32;
            // Base-dark → tip-light gradient, plus the per-blade shade jitter.
            let mut col = lerp_rgb(p.base_color, p.tip_color, t);
            col = [
                (col[0] as i32 + shade).clamp(0, 255) as u8,
                (col[1] as i32 + shade).clamp(0, 255) as u8,
                (col[2] as i32 + shade).clamp(0, 255) as u8,
            ];
            // Draw the run centred on the curled column.
            let half = bw / 2;
            for dx in 0..bw {
                put(&mut buf, cx - half + dx, y, col);
            }
        }
    }
    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Count fully-opaque (alpha == 255) pixels in an RGBA buffer.
    fn opaque(buf: &[u8]) -> usize {
        buf.chunks_exact(4).filter(|px| px[3] == 255).count()
    }

    #[test]
    fn zero_density_is_transparent() {
        let p = GrassTileParams::default();
        let buf = rasterize_grass(&p, 0.0, 1);
        assert!(buf.iter().all(|&b| b == 0), "ρ=0 paints nothing");
    }

    #[test]
    fn deterministic_in_seed() {
        let p = GrassTileParams::default();
        assert_eq!(
            rasterize_grass(&p, 0.7, 42),
            rasterize_grass(&p, 0.7, 42),
            "same seed and ρ → identical tile"
        );
        assert_ne!(
            rasterize_grass(&p, 0.7, 42),
            rasterize_grass(&p, 0.7, 43),
            "a different seed lays the blades differently"
        );
    }

    #[test]
    fn taller_grass_covers_more() {
        // Same seed fixes blade x/floor/sway/height-factor, so only ρ scales the
        // heights — each blade is a taller superset, hence more coverage.
        let p = GrassTileParams::default();
        let low = opaque(&rasterize_grass(&p, 0.25, 7));
        let high = opaque(&rasterize_grass(&p, 0.95, 7));
        assert!(high > low, "denser ρ paints more blade pixels ({high} > {low})");
    }

    #[test]
    fn grass_fills_tile_and_spills_into_overflow() {
        // Blades scatter across the tile (not a floor line) and the tallest grow up
        // past the tile's top edge into the overflow band rendered over the cell
        // above. Top `overflow` rows = overflow band; the rest is the tile region.
        let p = GrassTileParams { blades: 60, ..GrassTileParams::default() };
        let buf = rasterize_grass(&p, 1.0, 3);
        let w = p.canvas;
        let row_opaque = |r: usize| (0..w).filter(|&c| buf[(r * w + c) * 4 + 3] == 255).count();
        // Tall blades reach into the overflow band (the upper neighbour).
        assert!(
            (0..p.overflow).any(|r| row_opaque(r) > 0),
            "tall blades spill into the overflow band"
        );
        // Grass stands in the upper half of the tile, not only on a bottom baseline.
        let upper_tile_row = p.overflow + 2; // just inside the tile's top edge
        assert!(row_opaque(upper_tile_row) > 0, "grass fills the tile, not just its floor");
    }

    #[test]
    fn modest_density_does_not_reach_the_far_overflow() {
        // At low ρ blades are short, so the very top of the overflow band stays
        // empty: grass grows up by degrees, it doesn't hang from the top.
        let p = GrassTileParams { blades: 60, ..GrassTileParams::default() };
        let buf = rasterize_grass(&p, 0.3, 3);
        let w = p.canvas;
        let row_opaque = |r: usize| (0..w).filter(|&c| buf[(r * w + c) * 4 + 3] == 255).count();
        assert_eq!(row_opaque(0), 0, "the far top of the overflow band is empty at ρ=0.3");
    }

    #[test]
    fn blades_taper_and_grade() {
        // Good shape + gradient: the base run of a tall blade is wider than its
        // tip, and the tip is lighter than the base. Use one blade so the lowest
        // and highest opaque rows belong to the same blade.
        let p = GrassTileParams {
            blades: 1,
            sway: 0,
            height_jitter: 0.0,
            ..GrassTileParams::default()
        };
        let buf = rasterize_grass(&p, 1.0, 9);
        let n = p.canvas;
        let opaque_in_row =
            |r: usize| (0..n).filter(|&c| buf[(r * n + c) * 4 + 3] == 255).count();
        let green_in_row = |r: usize| {
            (0..n)
                .filter(|&c| buf[(r * n + c) * 4 + 3] == 255)
                .map(|c| buf[(r * n + c) * 4 + 1] as u32)
                .next()
                .unwrap_or(0)
        };
        let rows: Vec<usize> = (0..n).filter(|&r| opaque_in_row(r) > 0).collect();
        let top = *rows.first().unwrap(); // image row 0 is the tip
        let bottom = *rows.last().unwrap(); // higher row index is the base
        assert!(
            opaque_in_row(bottom) > opaque_in_row(top),
            "base run ({}) wider than tip run ({})",
            opaque_in_row(bottom),
            opaque_in_row(top)
        );
        assert!(
            green_in_row(top) > green_in_row(bottom),
            "tip ({}) lighter green than base ({})",
            green_in_row(top),
            green_in_row(bottom)
        );
    }

    #[test]
    fn degenerate_canvas_is_empty() {
        let p = GrassTileParams { canvas: 0, ..GrassTileParams::default() };
        assert!(rasterize_grass(&p, 1.0, 1).is_empty(), "0-canvas → no pixels");
    }
}
