//! Procedural flower-patch raster: scatters chunky blossoms on a transparent tile.
//! Renders only the colour it is handed — no second hue is introduced.

use rand::Rng;
use rand::SeedableRng;
use rand::rngs::StdRng;

/// Tunables for one flower patch.
pub struct FlowerPatchParams {
    pub canvas: usize,
    pub blossoms: usize,
    pub petal: usize,
    pub arm: usize,
    pub margin: usize,
}

impl Default for FlowerPatchParams {
    fn default() -> Self {
        Self {
            canvas: 32,
            blossoms: 4,
            petal: 5,
            arm: 3,
            margin: 7,
        }
    }
}

fn toward(c: [u8; 3], target: [u8; 3], k: f32) -> [u8; 3] {
    let k = k.clamp(0.0, 1.0);
    let mix = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * k).round() as u8;
    [mix(c[0], target[0]), mix(c[1], target[1]), mix(c[2], target[2])]
}

/// Rasterize a flower patch into RGBA8: `canvas*canvas*4` bytes. Background stays transparent.
/// Deterministic in `seed`.
pub fn rasterize_flower_patch(p: &FlowerPatchParams, color: [u8; 3], seed: u64) -> Vec<u8> {
    let n = p.canvas;
    let mut buf = vec![0u8; n * n * 4];
    if n == 0 || p.blossoms == 0 {
        return buf;
    }
    let mut rng = StdRng::seed_from_u64(seed);

    let petal_base = toward(color, [255, 255, 255], 0.15);

    let block = |buf: &mut [u8], cx: i32, cy: i32, size: usize, col: [u8; 3]| {
        let half = size as i32 / 2;
        for dy in 0..size as i32 {
            for dx in 0..size as i32 {
                let x = cx - half + dx;
                let y = cy - half + dy;
                if x < 0 || y < 0 || x >= n as i32 || y >= n as i32 {
                    continue;
                }
                let idx = (y as usize * n + x as usize) * 4;
                buf[idx] = col[0];
                buf[idx + 1] = col[1];
                buf[idx + 2] = col[2];
                buf[idx + 3] = 255;
            }
        }
    };

    let lo = p.margin as i32;
    let hi = (n as i32 - 1 - p.margin as i32).max(lo);
    for _ in 0..p.blossoms {
        let cx = rng.random_range(lo..=hi);
        let cy = rng.random_range(lo..=hi);
        let size = if rng.random::<bool>() {
            p.petal
        } else {
            p.petal.saturating_sub(1).max(1)
        };
        let arm = p.arm as i32;
        let shade = rng.random_range(-12i32..=12);
        let petal = [
            (petal_base[0] as i32 + shade).clamp(0, 255) as u8,
            (petal_base[1] as i32 + shade).clamp(0, 255) as u8,
            (petal_base[2] as i32 + shade).clamp(0, 255) as u8,
        ];
        block(&mut buf, cx, cy, size, petal);
        block(&mut buf, cx, cy - arm, size, petal);
        block(&mut buf, cx, cy + arm, size, petal);
        block(&mut buf, cx - arm, cy, size, petal);
        block(&mut buf, cx + arm, cy, size, petal);
    }
    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alphas(buf: &[u8]) -> (usize, usize) {
        let opaque = buf.chunks_exact(4).filter(|px| px[3] == 255).count();
        let clear = buf.chunks_exact(4).filter(|px| px[3] == 0).count();
        (opaque, clear)
    }

    #[test]
    fn patch_has_blossoms_on_clear_ground() {
        let p = FlowerPatchParams::default();
        let (opaque, clear) = alphas(&rasterize_flower_patch(&p, [255, 255, 255], 1));
        assert!(opaque > 0, "some blossom pixels are painted");
        assert!(clear > 0, "the background stays transparent");
    }

    #[test]
    fn deterministic_in_seed() {
        let p = FlowerPatchParams::default();
        assert_eq!(
            rasterize_flower_patch(&p, [0, 255, 255], 5),
            rasterize_flower_patch(&p, [0, 255, 255], 5),
            "same colour and seed → identical patch"
        );
        assert_ne!(
            rasterize_flower_patch(&p, [0, 255, 255], 5),
            rasterize_flower_patch(&p, [0, 255, 255], 6),
            "a different seed scatters the blossoms differently"
        );
    }

    #[test]
    fn renders_the_given_colour() {
        let p = FlowerPatchParams::default();
        let white = rasterize_flower_patch(&p, [255, 255, 255], 9);
        let cyan = rasterize_flower_patch(&p, [0, 255, 255], 9);
        assert_ne!(white, cyan, "the patch takes the colour it is handed");
    }

    #[test]
    fn introduces_no_new_hue() {
        let p = FlowerPatchParams::default();
        let buf = rasterize_flower_patch(&p, [0, 200, 200], 3);
        for px in buf.chunks_exact(4).filter(|px| px[3] == 255) {
            assert!(px[2] >= px[0], "blue ({}) stays >= red ({})", px[2], px[0]);
        }
    }

    #[test]
    fn degenerate_inputs_are_empty() {
        let zero_canvas = FlowerPatchParams { canvas: 0, ..FlowerPatchParams::default() };
        assert!(rasterize_flower_patch(&zero_canvas, [255, 255, 255], 1).is_empty());
        let no_blossoms = FlowerPatchParams { blossoms: 0, ..FlowerPatchParams::default() };
        let buf = rasterize_flower_patch(&no_blossoms, [255, 255, 255], 1);
        assert!(buf.iter().all(|&b| b == 0), "no blossoms → nothing painted");
    }
}
