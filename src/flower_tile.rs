//! Procedural flower-patch raster: scatters small blossoms across an otherwise
//! transparent tile, every blossom drawn in a single colour handed in from the
//! cell's position (the river-distance white→cyan ramp). The patch has no fixed
//! hue of its own — it renders only the signal it is given, so the colour stays
//! the layer's responsibility and the shape stays this module's. Pure and seeded
//! so the scatter is pinned by tests and reproducible per cell.
//!
//! A blossom is a chunky blob: a centre block with four petals at the orthogonals,
//! all the given colour lifted slightly toward white. It is deliberately big and
//! simple — a solid mass of colour, not a detailed daisy — so the position signal
//! (white→cyan) stays legible when the map is zoomed out. No second hue is ever
//! introduced (no yellow); the patch only renders the colour it is handed.

use rand::Rng;
use rand::SeedableRng;
use rand::rngs::StdRng;

/// Tunables for one flower patch.
pub struct FlowerPatchParams {
    /// Square tile resolution, in pixels.
    pub canvas: usize,
    /// Number of blossoms scattered across the tile.
    pub blossoms: usize,
    /// Petal/core block size, px (a blossom's largest blocks).
    pub petal: usize,
    /// Distance from a blossom's centre to each petal block, px.
    pub arm: usize,
    /// Keep blossom centres this far from the tile edge, px.
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

/// Linear blend of `c` toward `target` by `k` in `[0, 1]`.
fn toward(c: [u8; 3], target: [u8; 3], k: f32) -> [u8; 3] {
    let k = k.clamp(0.0, 1.0);
    let mix = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * k).round() as u8;
    [mix(c[0], target[0]), mix(c[1], target[1]), mix(c[2], target[2])]
}

/// Rasterize a flower patch into RGBA8 bytes: `canvas*canvas*4`, row-major. Every
/// blossom is drawn from `color` (petals lifted toward white, core toward black);
/// the background stays transparent. Deterministic in `seed`.
pub fn rasterize_flower_patch(p: &FlowerPatchParams, color: [u8; 3], seed: u64) -> Vec<u8> {
    let n = p.canvas;
    let mut buf = vec![0u8; n * n * 4];
    if n == 0 || p.blossoms == 0 {
        return buf;
    }
    let mut rng = StdRng::seed_from_u64(seed);

    // Lift the given colour a touch toward white so blossoms read as bright
    // blooms, but keep it close to the source so the position signal survives.
    let petal_base = toward(color, [255, 255, 255], 0.15);

    // Fill a `size`×`size` block centred on (cx, cy) in the given colour.
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
        // Size variation between blossoms → levels of scale.
        let size = if rng.random::<bool>() {
            p.petal
        } else {
            p.petal.saturating_sub(1).max(1)
        };
        let arm = p.arm as i32;
        // A small per-blossom shade so neighbouring blooms aren't identical.
        let shade = rng.random_range(-12i32..=12);
        let petal = [
            (petal_base[0] as i32 + shade).clamp(0, 255) as u8,
            (petal_base[1] as i32 + shade).clamp(0, 255) as u8,
            (petal_base[2] as i32 + shade).clamp(0, 255) as u8,
        ];
        // A solid blob: centre block plus four petals at the orthogonals, all one
        // colour so the bloom reads as a clean mass of the position signal.
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
        // Handed pure cyan (no red), every painted pixel keeps blue >= red: the
        // patch only lifts/darkens the given colour, it never invents a hue (e.g.
        // yellow, which would push red above blue).
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
