//! The half-block canvas — a truecolor pixel buffer that prints as terminal text.
//!
//! A terminal cell is about twice as tall as it is wide, so a sphere drawn one
//! pixel per cell comes out an egg. The canvas instead treats each cell as two
//! stacked pixels and prints the upper-half block `▀`: the glyph's *foreground*
//! color is the top pixel, its *background* color is the bottom. That doubles
//! vertical resolution and squares the pixels, so circles read as round.
//!
//! Drawing happens in pixel space (`cols` wide by `2 * rows` tall). `render`
//! turns the buffer into one ANSI frame, emitting a color escape only when it
//! changes from the previous cell to keep the byte count down.

use crate::driftscape::color::Rgb;
use std::fmt::Write;

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
            let mut last_bg: Option<Rgb> = None;
            for c in 0..self.cols {
                let top = self.buf[(2 * r) * self.cols + c];
                let bot = self.buf[(2 * r + 1) * self.cols + c];
                if last_fg != Some(top) {
                    let _ = write!(out, "\x1b[38;2;{};{};{}m", top.0, top.1, top.2);
                    last_fg = Some(top);
                }
                if last_bg != Some(bot) {
                    let _ = write!(out, "\x1b[48;2;{};{};{}m", bot.0, bot.1, bot.2);
                    last_bg = Some(bot);
                }
                out.push('▀');
            }
            out.push_str("\x1b[0m"); // reset so the background doesn't bleed past the row
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
    fn dimensions_and_clear() {
        let mut c = Canvas::new(4, 3);
        assert_eq!(c.ph(), 6);
        c.clear((1, 2, 3));
        // Every pixel reads back the clear color.
        let mut s = String::new();
        c.render(&mut s);
        assert!(s.contains("\x1b[38;2;1;2;3m"));
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
        c.line(1, 1, 10, 10, (7, 7, 7));
        let mut s = String::new();
        c.render(&mut s);
        // The in-bounds endpoint was painted (a non-default color appears)...
        assert!(s.contains("\x1b[38;2;7;7;7m") || s.contains("\x1b[48;2;7;7;7m"));
        // ...and running off the edge must not panic (reuses put's guard).
        c.line(-5, -5, 1, 1, (3, 3, 3));
        c.line(3, 3, 3, 3, (5, 5, 5)); // degenerate: single pixel, in bounds
    }

    #[test]
    fn render_has_one_block_per_cell() {
        let mut c = Canvas::new(3, 2);
        c.clear((0, 0, 0));
        let mut s = String::new();
        c.render(&mut s);
        // 3 cols * 2 rows = 6 half-block glyphs.
        assert_eq!(s.matches('▀').count(), 6);
    }
}
