//! Soil-type layer: derives a riparian/steppe gradient from the water-proximity
//! field the water layer already authored. Near-water cells trend toward 1
//! (riparian — dark, moist), far cells stay at 0 (steppe — pale, dry).

use crate::grid::Grid;

const LO: f32 = 0.2;
const HI: f32 = 0.7;

/// Derive `soil_type` for every cell from the water-proximity gradient.
/// Must run after `generate_water` and before `seed_browse_cap`.
pub(super) fn seed_soil_type(grid: &mut Grid) {
    for i in 0..grid.len() {
        let t = ((grid.water_prox(i) - LO) / (HI - LO)).clamp(0.0, 1.0);
        grid.set_soil_type(i, t);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::Grid;

    #[test]
    fn riparian_hugs_water() {
        let mut grid = Grid::new(4, 4);
        // High water_prox on cells 0,1 (near water) and low on cells 2,3.
        grid.set_water_prox(0, 0.9);
        grid.set_water_prox(1, 0.8);
        grid.set_water_prox(2, 0.1);
        grid.set_water_prox(3, 0.0);

        seed_soil_type(&mut grid);

        assert!(grid.soil_type(0) > grid.soil_type(2),
            "near-water cell should be more riparian than far cell");
        assert!(grid.soil_type(1) > grid.soil_type(3),
            "near-water cell should be more riparian than far cell");

        for i in 0..grid.len() {
            let v = grid.soil_type(i);
            assert!((0.0..=1.0).contains(&v), "soil_type out of range: {v}");
        }
    }
}
