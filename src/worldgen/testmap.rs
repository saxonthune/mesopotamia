//! Hand-built grids for exercising behaviour on controlled terrain. A `WorldSource::TestMap`
//! diverts `generate_world` to one of these builders instead of the procedural layers, so a
//! map is a named `fn(&mut Grid)` plus the shape primitives it draws with.

use bevy::prelude::Resource;

use crate::grid::{Grid, MAX_SHRUBS};

/// One named controlled map: the grid is sized to `width × height`, reset, then `build` paints it.
pub struct TestMap {
    pub name: &'static str,
    pub description: &'static str,
    pub width: usize,
    pub height: usize,
    pub build: fn(&mut Grid),
}

/// Selects what `generate_world` paints. Default keeps the procedural world; the `--test-map`
/// flag overrides it before the first tick.
#[derive(Resource, Default, Clone, Copy)]
pub enum WorldSource {
    #[default]
    Procedural,
    TestMap(&'static TestMap),
}

// Four 20×10 shrub ovals (gaps 2/8/20), 4 bare cols of margin each side, 10 bare rows each side.
const OVAL_HALF_W: usize = 10;
const OVAL_HALF_H: usize = 5;
const OVAL_GAPS: [usize; 3] = [2, 8, 20];
const PAD_X: usize = 4;
const PAD_Y: usize = 10;

const OVAL_GAPS_W: usize =
    4 * (2 * OVAL_HALF_W + 1) + OVAL_GAPS[0] + OVAL_GAPS[1] + OVAL_GAPS[2] + 2 * PAD_X;
const OVAL_GAPS_H: usize = (2 * OVAL_HALF_H + 1) + 2 * PAD_Y;

pub const TEST_MAPS: &[TestMap] = &[TestMap {
    name: "oval-gaps",
    description: "Four 20×10 shrub ovals in a row over bare ground, gaps of 2/8/20 cells — \
                  a crossing test over widening bare ground.",
    width: OVAL_GAPS_W,
    height: OVAL_GAPS_H,
    build: oval_gaps,
}];

pub fn find(name: &str) -> Option<&'static TestMap> {
    TEST_MAPS.iter().find(|m| m.name == name)
}

fn names() -> String {
    TEST_MAPS.iter().map(|m| m.name).collect::<Vec<_>>().join(", ")
}

/// Resolve `--test-map <name>` / `--test-map=<name>` from CLI args into a `WorldSource`.
/// Absent flag → `Procedural`; an unknown name → `Err` listing the registered maps.
pub fn source_from_args<I: IntoIterator<Item = String>>(args: I) -> Result<WorldSource, String> {
    let mut it = args.into_iter();
    while let Some(arg) = it.next() {
        let name = if let Some(rest) = arg.strip_prefix("--test-map=") {
            Some(rest.to_string())
        } else if arg == "--test-map" {
            Some(it.next().ok_or_else(|| format!("--test-map needs a name (one of: {})", names()))?)
        } else {
            None
        };
        if let Some(name) = name {
            let map = find(&name).ok_or_else(|| format!("unknown test map {name:?} (one of: {})", names()))?;
            return Ok(WorldSource::TestMap(map));
        }
    }
    Ok(WorldSource::Procedural)
}

/// Fill the axis-aligned ellipse centred at `(cc, cr)` with half-extents `(hw, hh)` to full
/// shrub cover. A cell is inside when `(dc/hw)² + (dr/hh)² ≤ 1`.
pub fn shrub_oval(grid: &mut Grid, cc: usize, cr: usize, hw: usize, hh: usize) {
    let (width, height) = (grid.width(), grid.height());
    let col_lo = cc.saturating_sub(hw);
    let col_hi = (cc + hw).min(width.saturating_sub(1));
    let row_lo = cr.saturating_sub(hh);
    let row_hi = (cr + hh).min(height.saturating_sub(1));
    for row in row_lo..=row_hi {
        for col in col_lo..=col_hi {
            let dc = (col as f32 - cc as f32) / hw as f32;
            let dr = (row as f32 - cr as f32) / hh as f32;
            if dc * dc + dr * dr <= 1.0 {
                let i = row * width + col;
                grid.set_shrub_cap(i, MAX_SHRUBS);
                grid.set_shrubs(i, MAX_SHRUBS);
            }
        }
    }
}

/// A vertically-centred, horizontally-centred row of equal `2·hw × 2·hh` shrub ovals separated
/// left-to-right by `gaps` bare columns each — one oval more than there are gaps.
pub fn shrub_oval_row(grid: &mut Grid, hw: usize, hh: usize, gaps: &[usize]) {
    let count = gaps.len() + 1;
    // An ellipse of half-width `hw` paints `2·hw + 1` cells across its centre row (inclusive
    // endpoints), so centre-to-centre spacing is that width plus the bare gap.
    let oval_w = 2 * hw + 1;
    let span = count * oval_w + gaps.iter().sum::<usize>();
    let mut col = grid.width().saturating_sub(span) / 2 + hw;
    let cr = grid.height() / 2;
    for n in 0..count {
        shrub_oval(grid, col, cr, hw, hh);
        col += oval_w;
        if n < gaps.len() {
            col += gaps[n];
        }
    }
}

/// Zero grass capacity everywhere so nothing greens up — only the painted shrubs are food.
pub fn bare_ground(grid: &mut Grid) {
    for i in 0..grid.len() {
        grid.set_water_prox(i, 0.0);
    }
}

fn oval_gaps(grid: &mut Grid) {
    bare_ground(grid);
    shrub_oval_row(grid, OVAL_HALF_W, OVAL_HALF_H, &OVAL_GAPS);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_resolves_known_and_rejects_unknown() {
        assert!(find("oval-gaps").is_some());
        assert!(find("nope").is_none());
    }

    fn args(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn source_from_args_parses_flag_forms() {
        assert!(matches!(source_from_args(args(&["bin"])), Ok(WorldSource::Procedural)));
        assert!(matches!(
            source_from_args(args(&["bin", "--test-map", "oval-gaps"])),
            Ok(WorldSource::TestMap(_))
        ));
        assert!(matches!(
            source_from_args(args(&["bin", "--test-map=oval-gaps"])),
            Ok(WorldSource::TestMap(_))
        ));
        assert!(source_from_args(args(&["bin", "--test-map", "bogus"])).is_err());
        assert!(source_from_args(args(&["bin", "--test-map"])).is_err());
    }

    #[test]
    fn shrub_oval_fills_centre_and_clears_corners() {
        let mut grid = Grid::new(40, 40);
        shrub_oval(&mut grid, 20, 20, 10, 5);
        let centre = 20 * 40 + 20;
        assert_eq!(grid.shrubs(centre), MAX_SHRUBS, "centre is full shrub");
        let corner = (20 - 5) * 40 + (20 - 10); // bounding-box corner sits outside the ellipse
        assert_eq!(grid.shrubs(corner), 0.0, "ellipse corner stays bare");
    }

    fn map_grid() -> Grid {
        let m = &TEST_MAPS[0];
        let mut grid = Grid::new(m.width, m.height);
        (m.build)(&mut grid);
        grid
    }

    #[test]
    fn oval_gaps_is_bare_off_the_shrubs() {
        let grid = map_grid();
        // No cell can grow grass (capacity zero), and a corner cell carries no shrub.
        assert_eq!(grid.capacity(0), 0.0, "bare ground has zero grass capacity");
        assert_eq!(grid.shrubs(0), 0.0, "corner is bare");
    }

    #[test]
    fn oval_gaps_lays_out_widening_gaps_and_padding() {
        let grid = map_grid();
        let (w, h) = (grid.width(), grid.height());
        let cr = h / 2;
        let row: Vec<bool> = (0..w).map(|c| grid.shrubs(cr * w + c) > 0.0).collect();

        let mut runs_shrub = Vec::new();
        let mut runs_gap = Vec::new();
        let mut i = 0;
        while i < row.len() && !row[i] { i += 1; }
        let left_pad = i;
        while i < row.len() {
            let start = i;
            while i < row.len() && row[i] { i += 1; }
            runs_shrub.push(i - start);
            let gap_start = i;
            while i < row.len() && !row[i] { i += 1; }
            if i < row.len() { runs_gap.push(i - gap_start); }
        }
        let right_pad = row.len() - row.iter().rposition(|&s| s).unwrap() - 1;

        assert_eq!(runs_shrub.len(), 4, "four ovals on the centre row");
        assert_eq!(runs_gap, vec![2, 8, 20], "gaps widen 2/8/20");
        assert_eq!((left_pad, right_pad), (PAD_X, PAD_X), "4 bare columns each side");

        // Vertical padding: PAD_Y bare rows above the first shrub column of cells.
        let cc = left_pad + OVAL_HALF_W; // centre of the first oval
        let top_pad = (0..h).take_while(|&r| grid.shrubs(r * w + cc) == 0.0).count();
        assert_eq!(top_pad, PAD_Y, "10 bare rows above the patches");
    }
}
