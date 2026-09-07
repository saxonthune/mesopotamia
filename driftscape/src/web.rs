//! Browser adapter: exposes the Starliner scene as a flat cell buffer that the
//! HTML canvas paints directly, bypassing ANSI serialization and terminal
//! emulation entirely.

use wasm_bindgen::prelude::*;

use crate::canvas::Canvas;
use crate::scene::Scene;
use crate::starliner::Starliner;

#[wasm_bindgen]
pub struct Planets {
    cols: usize,
    rows: usize,
    seed: u64,
    scene: Starliner,
    canvas: Canvas,
    cells: Vec<u8>,
}

#[wasm_bindgen]
impl Planets {
    #[wasm_bindgen(constructor)]
    pub fn new(cols: u32, rows: u32, seed: u32) -> Self {
        let cols = valid_dimension(cols);
        let rows = valid_dimension(rows);
        let seed = seed as u64;
        Self {
            cols,
            rows,
            seed,
            scene: Starliner::new(cols, rows, seed),
            canvas: Canvas::new(cols, rows),
            cells: Vec::with_capacity(cols * rows * 4),
        }
    }

    pub fn resize(&mut self, cols: u32, rows: u32) {
        let cols = valid_dimension(cols);
        let rows = valid_dimension(rows);
        if (cols, rows) == (self.cols, self.rows) {
            return;
        }
        self.cols = cols;
        self.rows = rows;
        self.scene = Starliner::new(cols, rows, self.seed);
        self.canvas = Canvas::new(cols, rows);
    }

    /// Advance one step and return a pointer to the cell buffer. Each cell is
    /// four bytes: `[glyph_ascii, r, g, b]`, row-major, `cols * rows` cells.
    pub fn render(&mut self, dt: f32) -> *const u8 {
        self.scene.step(dt.clamp(0.0, 0.1));
        self.scene.draw(&mut self.canvas);
        self.canvas.render_cells(&mut self.cells);
        self.cells.as_ptr()
    }

    pub fn cell_count(&self) -> usize {
        self.cols * self.rows
    }

    pub fn get_cols(&self) -> usize {
        self.cols
    }

    pub fn get_rows(&self) -> usize {
        self.rows
    }
}

fn valid_dimension(value: u32) -> usize {
    value.clamp(1, 500) as usize
}
