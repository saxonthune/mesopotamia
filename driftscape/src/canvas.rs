//! The ASCII canvas — a truecolor pixel buffer that prints as terminal glyphs.
//!
//! A terminal cell is about twice as tall as it is wide, so drawing happens in
//! an oversampled pixel space (`cols` wide by `2 * rows` tall) where pixels are
//! square — a sphere there is round, not an egg. `render` then collapses each
//! cell's two stacked pixels into a single character: the pixels' combined
//! brightness chooses a glyph from a dark-to-light ramp (` .,:;=+*#%@`) and the
//! brighter pixel lends its truecolor to the glyph. The result is colored ASCII
//! art rather than solid color blocks — the field reads as characters.
//!
//! A color escape is emitted only when the foreground changes from the previous
//! cell, to keep the byte count down.

use crate::color::Rgb;
use std::fmt::Write;

/// Dark-to-light glyph ramp. Index 0 is the empty field; the last is the
/// brightest core. Brightness maps linearly onto these levels.
const RAMP: &[u8] = b" .,:;=+*#%@";

/// Brightness of a color in `0.0..=1.0`, taken as its brightest channel (HSV
/// value). Using value rather than perceptual luminance keeps a fully-lit but
/// saturated color — a pure red planet — reading as a *dense* glyph instead of
/// a sparse one, so colored bodies stay solid rather than holey.
fn brightness(c: Rgb) -> f32 {
    c.0.max(c.1).max(c.2) as f32 / 255.0
}

/// Collapse a cell's two stacked pixels into the glyph and color to print: the
/// glyph comes from the mean brightness, the color from the brighter pixel so a
/// lit edge keeps its hue. Returns `(glyph, foreground)`.
fn cell(top: Rgb, bot: Rgb) -> (char, Rgb) {
    let (lt, lb) = (brightness(top), brightness(bot));
    let level = ((lt + lb) * 0.5 * (RAMP.len() - 1) as f32).round() as usize;
    let glyph = RAMP[level.min(RAMP.len() - 1)] as char;
    let color = if lt >= lb { top } else { bot };
    (glyph, color)
}

pub struct Canvas {
    /// Terminal columns, which is also the pixel width.
    pub cols: usize,
    /// Terminal rows. Pixel height is `2 * rows`.
    pub rows: usize,
    buf: Vec<Rgb>,
}

impl Canvas {
    pub fn new(cols: usize, rows: usize) -> Self {
        Self { cols, rows, buf: vec![(0, 0, 0); cols * rows * 2] }
    }

    /// Pixel height: two stacked pixels per terminal row.
    pub fn ph(&self) -> usize {
        self.rows * 2
    }

    /// Fill the whole buffer with one color (the space background each frame).
    pub fn clear(&mut self, c: Rgb) {
        self.buf.iter_mut().for_each(|p| *p = c);
    }

    /// Set one pixel. Out-of-bounds coordinates are silently dropped, so callers
    /// can draw shapes that straddle an edge without clipping by hand.
    pub fn put(&mut self, x: i32, y: i32, c: Rgb) {
        if x < 0 || y < 0 {
            return;
        }
        let (x, y) = (x as usize, y as usize);
        if x >= self.cols || y >= self.ph() {
            return;
        }
        self.buf[y * self.cols + x] = c;
    }

    /// Draw a straight line between two pixels (integer Bresenham). Each step
    /// goes through the bounds-checked `put`, so a segment that runs off the
    /// edge is clipped pixel-by-pixel rather than clamped. Used for star streaks.
    pub fn line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, c: Rgb) {
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        let (mut x, mut y) = (x0, y0);
        loop {
            self.put(x, y, c);
            if x == x1 && y == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
        }
    }

    /// Append one full frame of ANSI to `out`. Caller is responsible for clearing
    /// `out` first if reusing the buffer across frames.
    pub fn render(&self, out: &mut String) {
        out.push_str("\x1b[H"); // cursor home
        for r in 0..self.rows {
            let mut last_fg: Option<Rgb> = None;
            for c in 0..self.cols {
                let top = self.buf[(2 * r) * self.cols + c];
                let bot = self.buf[(2 * r + 1) * self.cols + c];
                let (glyph, fg) = cell(top, bot);
                // A blank cell needs no color — skip the escape and let it ride.
                if glyph != ' ' && last_fg != Some(fg) {
                    let _ = write!(out, "\x1b[38;2;{};{};{}m", fg.0, fg.1, fg.2);
                    last_fg = Some(fg);
                }
                out.push(glyph);
            }
            out.push_str("\x1b[0m"); // reset so color doesn't bleed past the row
            if r + 1 < self.rows {
                out.push_str("\r\n");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dimensions_and_dark_clear_is_blank() {
        let mut c = Canvas::new(4, 3);
        assert_eq!(c.ph(), 6);
        c.clear((1, 2, 3)); // near-black background
        let mut s = String::new();
        c.render(&mut s);
        // A dark field maps to the empty ramp glyph, emitting no color escapes.
        assert!(!s.contains("\x1b[38;2;"));
        assert_eq!(s.matches(' ').count(), 4 * 3);
    }

    #[test]
    fn bright_clear_picks_the_densest_glyph_and_color() {
        let mut c = Canvas::new(2, 2);
        c.clear((255, 255, 255));
        let mut s = String::new();
        c.render(&mut s);
        assert!(s.contains("\x1b[38;2;255;255;255m"));
        assert_eq!(s.matches('@').count(), 2 * 2);
    }

    #[test]
    fn cell_glyph_climbs_the_ramp_with_brightness() {
        // Brighter pixels select later (denser) glyphs in the ramp.
        let (dim, _) = cell((20, 20, 20), (20, 20, 20));
        let (mid, _) = cell((128, 128, 128), (128, 128, 128));
        let (hot, _) = cell((255, 255, 255), (255, 255, 255));
        let pos = |g: char| RAMP.iter().position(|&b| b as char == g).unwrap();
        assert!(pos(dim) < pos(mid) && pos(mid) < pos(hot));
    }

    #[test]
    fn cell_color_follows_the_brighter_pixel() {
        // The lit pixel lends its hue even when stacked over a dark one.
        let (_, fg) = cell((200, 40, 40), (5, 5, 5));
        assert_eq!(fg, (200, 40, 40));
    }

    #[test]
    fn put_ignores_out_of_bounds() {
        let mut c = Canvas::new(2, 2);
        // None of these should panic.
        c.put(-1, 0, (9, 9, 9));
        c.put(0, -1, (9, 9, 9));
        c.put(2, 0, (9, 9, 9));
        c.put(0, 4, (9, 9, 9));
        c.put(1, 3, (5, 5, 5)); // in bounds (ph == 4)
    }

    #[test]
    fn line_paints_endpoints_and_clips_off_edge() {
        let mut c = Canvas::new(4, 2); // 4 wide, ph == 4 tall
        // A segment that starts in bounds and runs off the right/bottom edge.
        c.line(1, 1, 10, 10, (200, 200, 200));
        let mut s = String::new();
        c.render(&mut s);
        // The in-bounds endpoint was painted (a lit glyph's color appears)...
        assert!(s.contains("\x1b[38;2;200;200;200m"));
        // ...and running off the edge must not panic (reuses put's guard).
        c.line(-5, -5, 1, 1, (3, 3, 3));
        c.line(3, 3, 3, 3, (5, 5, 5)); // degenerate: single pixel, in bounds
    }

    #[test]
    fn render_has_one_glyph_per_cell() {
        let mut c = Canvas::new(3, 2);
        c.clear((255, 255, 255));
        let mut s = String::new();
        c.render(&mut s);
        // 3 cols * 2 rows = 6 glyphs; a full-white field is all '@'.
        assert_eq!(s.matches('@').count(), 6);
    }
}
