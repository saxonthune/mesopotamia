//! Color math for Driftscape — pure RGB helpers shared by every scene.

/// 8-bit truecolor, the unit the half-block canvas composites in.
pub type Rgb = (u8, u8, u8);

/// HSV to RGB. `h` is degrees (wrapped into `0..360`), `s` and `v` are `0..=1`.
/// Scenes pick a hue per object and let saturation/value carry the shading.
pub fn hsv_to_rgb(h: f32, s: f32, v: f32) -> Rgb {
    let h = h.rem_euclid(360.0) / 60.0;
    let c = v * s.clamp(0.0, 1.0);
    let x = c * (1.0 - ((h % 2.0) - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match h as i32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let to_u8 = |f: f32| ((f + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    (to_u8(r), to_u8(g), to_u8(b))
}

/// Multiply a color's brightness by `f` (clamped per channel). Used to dim a
/// base color across a sphere's lit-to-dark falloff.
pub fn scale(c: Rgb, f: f32) -> Rgb {
    let m = |v: u8| (v as f32 * f.max(0.0)).round().clamp(0.0, 255.0) as u8;
    (m(c.0), m(c.1), m(c.2))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hue_wraps_and_hits_primaries() {
        assert_eq!(hsv_to_rgb(0.0, 1.0, 1.0), (255, 0, 0));
        assert_eq!(hsv_to_rgb(120.0, 1.0, 1.0), (0, 255, 0));
        assert_eq!(hsv_to_rgb(240.0, 1.0, 1.0), (0, 0, 255));
        // 360 wraps back to red.
        assert_eq!(hsv_to_rgb(360.0, 1.0, 1.0), hsv_to_rgb(0.0, 1.0, 1.0));
        // Negative hue wraps too.
        assert_eq!(hsv_to_rgb(-120.0, 1.0, 1.0), hsv_to_rgb(240.0, 1.0, 1.0));
    }

    #[test]
    fn scale_dims_and_clamps() {
        assert_eq!(scale((100, 200, 50), 0.5), (50, 100, 25));
        assert_eq!(scale((200, 200, 200), 2.0), (255, 255, 255));
        assert_eq!(scale((100, 100, 100), 0.0), (0, 0, 0));
    }
}
