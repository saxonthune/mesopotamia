//! Procedural grass-tile raster: fills a `canvas × (canvas+overflow)` RGBA canvas with
//! grass blades scaled by ρ. The overflow band is rendered over the cell above so blades spill upward.

use rand::Rng;
use rand::SeedableRng;
use rand::rngs::StdRng;

/// Tunables for one grass tile.
pub struct GrassTileParams {
    pub canvas: usize,
    pub overflow: usize,
    pub blades: usize,
    pub base_width: usize,
    pub base_min: usize,
    pub base_max: usize,
    pub blade_len: usize,
    pub back_shrink: f32,
    pub sway: i32,
    pub height_jitter: f32,
    pub base_color: [u8; 3],
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

fn lerp_rgb(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 3] {
    let t = t.clamp(0.0, 1.0);
    let mix = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    [mix(a[0], b[0]), mix(a[1], b[1]), mix(a[2], b[2])]
}

/// Rasterize a grass tile into RGBA8: `canvas*(canvas+overflow)*4` bytes, row-major top-left.
/// Bottom `canvas` rows = tile; top `overflow` rows = band rendered over the cell above.
/// `rho=0` → transparent. Deterministic in `seed`.
pub fn rasterize_grass(p: &GrassTileParams, rho: f32, seed: u64) -> Vec<u8> {
    let w = p.canvas;
    let h_full = p.canvas + p.overflow;
    let mut buf = vec![0u8; w * h_full * 4];
    let rho = rho.clamp(0.0, 1.0);
    if rho <= 0.0 || w == 0 {
        return buf;
    }
    let mut rng = StdRng::seed_from_u64(seed);
    let root_lo = p.base_min.min(w.saturating_sub(1));
    let root_hi = p.base_max.min(w.saturating_sub(1)).max(root_lo);
    let base_w = p.base_width.max(1) as f32;

    let put = |buf: &mut [u8], cx: i32, y: usize, col: [u8; 3]| {
        if cx < 0 || cx >= w as i32 || y >= h_full {
            return;
        }
        let row = h_full - 1 - y; // y=0 is the tile bottom; image row 0 is the overflow-band top
        let idx = (row * w + cx as usize) * 4;
        buf[idx] = col[0];
        buf[idx + 1] = col[1];
        buf[idx + 2] = col[2];
        buf[idx + 3] = 255;
    };

    for _ in 0..p.blades {
        let margin = (base_w as i32 / 2).max(1);
        let lo = margin;
        let hi = (w as i32 - 1 - margin).max(lo);
        let x0 = rng.random_range(lo..=hi);
        let base = if root_hi > root_lo {
            rng.random_range(root_lo..=root_hi)
        } else {
            root_lo
        };
        let depth = 1.0 - p.back_shrink * (base as f32 / w as f32);
        let hfac = 1.0 - p.height_jitter * rng.random::<f32>();
        let height = ((rho * p.blade_len as f32 * hfac * depth).round() as usize).max(1);
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
            let t = h as f32 / height as f32;
            let cx = x0 + (sway * t * t).round() as i32;
            let bw = (base_w * (1.0 - 0.7 * t)).round().max(1.0) as i32;
            let mut col = lerp_rgb(p.base_color, p.tip_color, t);
            col = [
                (col[0] as i32 + shade).clamp(0, 255) as u8,
                (col[1] as i32 + shade).clamp(0, 255) as u8,
                (col[2] as i32 + shade).clamp(0, 255) as u8,
            ];
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
        let p = GrassTileParams::default();
        let low = opaque(&rasterize_grass(&p, 0.25, 7));
        let high = opaque(&rasterize_grass(&p, 0.95, 7));
        assert!(high > low, "denser ρ paints more blade pixels ({high} > {low})");
    }

    #[test]
    fn grass_fills_tile_and_spills_into_overflow() {
        let p = GrassTileParams { blades: 60, ..GrassTileParams::default() };
        let buf = rasterize_grass(&p, 1.0, 3);
        let w = p.canvas;
        let row_opaque = |r: usize| (0..w).filter(|&c| buf[(r * w + c) * 4 + 3] == 255).count();
        assert!(
            (0..p.overflow).any(|r| row_opaque(r) > 0),
            "tall blades spill into the overflow band"
        );
        let upper_tile_row = p.overflow + 2;
        assert!(row_opaque(upper_tile_row) > 0, "grass fills the tile, not just its floor");
    }

    #[test]
    fn modest_density_does_not_reach_the_far_overflow() {
        let p = GrassTileParams { blades: 60, ..GrassTileParams::default() };
        let buf = rasterize_grass(&p, 0.3, 3);
        let w = p.canvas;
        let row_opaque = |r: usize| (0..w).filter(|&c| buf[(r * w + c) * 4 + 3] == 255).count();
        assert_eq!(row_opaque(0), 0, "the far top of the overflow band is empty at ρ=0.3");
    }

    #[test]
    fn blades_taper_and_grade() {
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
        let top = *rows.first().unwrap();
        let bottom = *rows.last().unwrap();
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
