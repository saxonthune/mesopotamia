//! Cosmetic soil-texture layer: a sparse reddish speckle that fills the dry soil
//! *around* the rough patches rather than scattering blind across the steppe.
//! Reading the already-placed rough centers (the unfolding method's binding rule),
//! a distance-to-rough field weights placement so the grain is densest hugging the
//! broken ground and fades to nothing in the open — the speckle reads as the
//! rough's apron, filling the interstices between nearby patches. Two tints, each
//! a hair toward mahogany: tint 1 as 1×2 horizontal pairs, tint 2 as lone 1×1
//! cells. Purely visual — render maps the tint id to a colour.

use std::collections::VecDeque;

use rand::Rng;
use rand::SeedableRng;
use rand::rngs::StdRng;

use crate::grid::Grid;

/// How far the apron reaches from a rough patch, in cells: placement weight falls
/// linearly from full right beside the rough to zero this many cells out.
const REACH: u32 = 10;
/// Base chance of a 1×2 tint-1 pair right beside a rough patch (scaled down by the
/// proximity weight away from it).
const PAIR_CHANCE: f32 = 0.06;
/// Base chance of a lone 1×1 tint-2 speckle right beside a rough patch.
const SINGLE_CHANCE: f32 = 0.10;

/// Whether a cell is open tan soil eligible for speckle: not water, not rough.
fn is_tan_soil(grid: &Grid, index: usize) -> bool {
    grid.water(index) == 0.0 && grid.rough(index) == 0.0
}

/// Multi-source BFS distance (4-connectivity) from every rough cell: 0 on a rough
/// cell, rising outward, `u32::MAX` where no rough is reachable. The field the
/// apron weight reads.
fn dist_to_rough(grid: &Grid) -> Vec<u32> {
    let len = grid.len();
    let mut dist = vec![u32::MAX; len];
    let mut queue = VecDeque::new();
    for i in 0..len {
        if grid.rough(i) > 0.0 {
            dist[i] = 0;
            queue.push_back(i);
        }
    }
    while let Some(idx) = queue.pop_front() {
        let next = dist[idx] + 1;
        for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
            if let Some(n) = grid.step(idx, dx, dy) && dist[n] == u32::MAX {
                dist[n] = next;
                queue.push_back(n);
            }
        }
    }
    dist
}

/// Apron weight in [0, 1] from a cell's distance to the nearest rough: full one
/// cell out, fading linearly to zero by `REACH`. Zero on a rough cell (dist 0) and
/// where no rough is reachable, so the open steppe stays clean.
fn proximity_weight(dist: u32) -> f32 {
    if dist == 0 || dist == u32::MAX {
        return 0.0;
    }
    (1.0 - (dist - 1) as f32 / REACH as f32).clamp(0.0, 1.0)
}

/// Scatter the two soil tints into the apron around the rough patches. Reads
/// `water`/`rough` so the speckle avoids channels and broken ground and clusters
/// near rough; `soil_texture_seed` keeps it independent and reproducible.
pub(super) fn seed_soil_texture(grid: &mut Grid, seed: u64) {
    let mut rng = StdRng::seed_from_u64(seed);
    let dist = dist_to_rough(grid);
    let (w, h) = (grid.width(), grid.height());
    for row in 0..h {
        for col in 0..w {
            let i = row * w + col;
            if grid.soil_tint(i) != 0 || !is_tan_soil(grid, i) {
                continue;
            }
            let weight = proximity_weight(dist[i]);
            if weight <= 0.0 {
                continue;
            }
            // Try a 1×2 pair first: stamp this cell and its right neighbour tint 1
            // when the neighbour is open soil and still untinted.
            if rng.random::<f32>() < PAIR_CHANCE * weight && col + 1 < w {
                let r = i + 1;
                if grid.soil_tint(r) == 0 && is_tan_soil(grid, r) {
                    grid.set_soil_tint(i, 1);
                    grid.set_soil_tint(r, 1);
                    continue;
                }
            }
            // Otherwise a lone 1×1 tint-2 speckle.
            if rng.random::<f32>() < SINGLE_CHANCE * weight {
                grid.set_soil_tint(i, 2);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::Grid;

    /// A patch of rough seeds an apron of speckle; the field is deterministic and
    /// never lands on water or rough cells.
    #[test]
    fn speckle_fills_apron_and_is_deterministic() {
        let build = || {
            let mut g = Grid::new(80, 48);
            // A small rough patch in the interior, plus a water cell beside it.
            for c in 18..24 {
                g.set_rough(24 * 80 + c, 1.0);
            }
            g.set_water(24 * 80 + 19, 1.0); // overlaps the patch row to test gating
            g
        };
        let mut g1 = build();
        let mut g2 = build();
        seed_soil_texture(&mut g1, 0xABCD);
        seed_soil_texture(&mut g2, 0xABCD);

        let t1: Vec<u8> = (0..g1.len()).map(|i| g1.soil_tint(i)).collect();
        let t2: Vec<u8> = (0..g2.len()).map(|i| g2.soil_tint(i)).collect();
        assert_eq!(t1, t2, "same seed → same speckle");
        for i in 0..g1.len() {
            if g1.rough(i) > 0.0 || g1.water(i) > 0.0 {
                assert_eq!(g1.soil_tint(i), 0, "speckle never lands on rough/water");
            }
        }
        assert!(t1.iter().any(|&t| t == 1), "some 1×2 tint-1 placed in the apron");
        assert!(t1.iter().any(|&t| t == 2), "some 1×1 tint-2 placed in the apron");
    }

    /// Speckle only appears within the apron — no tint lands farther than `REACH`
    /// from a rough patch, so the open steppe stays clean.
    #[test]
    fn speckle_stays_within_reach_of_rough() {
        let mut g = Grid::new(96, 40);
        for c in 10..14 {
            g.set_rough(20 * 96 + c, 1.0);
        }
        seed_soil_texture(&mut g, 0x55);
        let dist = dist_to_rough(&g);
        for i in 0..g.len() {
            if g.soil_tint(i) != 0 {
                assert!(
                    dist[i] >= 1 && dist[i] <= REACH,
                    "tint at {i} is {} cells from rough, outside the apron",
                    dist[i]
                );
            }
        }
    }

    /// With no rough at all, there is no apron and the steppe stays bare.
    #[test]
    fn no_rough_means_no_speckle() {
        let mut g = Grid::new(40, 32);
        seed_soil_texture(&mut g, 0x99);
        assert!((0..g.len()).all(|i| g.soil_tint(i) == 0), "no rough → no speckle");
    }

    #[test]
    fn weight_peaks_beside_rough_and_fades_to_zero() {
        assert_eq!(proximity_weight(0), 0.0, "a rough cell carries no apron weight");
        assert_eq!(proximity_weight(u32::MAX), 0.0, "unreachable cells weigh zero");
        assert!((proximity_weight(1) - 1.0).abs() < 1e-6, "adjacent cell is full weight");
        assert!(proximity_weight(1) > proximity_weight(5), "weight decays with distance");
        assert_eq!(proximity_weight(REACH + 1), 0.0, "beyond reach the weight is zero");
    }

    #[test]
    fn tint_one_always_has_a_horizontal_partner() {
        let mut g = Grid::new(80, 48);
        for c in 18..24 {
            g.set_rough(24 * 80 + c, 1.0);
        }
        seed_soil_texture(&mut g, 0x1234);
        let w = g.width();
        for i in 0..g.len() {
            if g.soil_tint(i) == 1 {
                let col = i % w;
                let left = col > 0 && g.soil_tint(i - 1) == 1;
                let right = col + 1 < w && g.soil_tint(i + 1) == 1;
                assert!(left || right, "tint-1 cell {i} lacks its 1×2 partner");
            }
        }
    }
}
