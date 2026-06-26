//! Procedural watershed generation. Submodules: `cost` (least-cost carve), `raster` (stamp + fords),
//! `features` (oxbows, tribs), `lake` (metaball lakes), `prox` (bank capacity). Seed-deterministic.

mod cost;
mod features;
mod lake;
mod prox;
mod raster;
mod spec;

pub use spec::{random_confluences, RiverSpec};

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use crate::grid::Grid;

use crate::field;
use cost::{carve, cost_field, lerp};
use raster::{stamp_main_channel, tag_shallows_as_fords};
use spec::{PEN_HIGH, PEN_LOW};

// Returns (mains, tribs) centerline paths; tests use them, orchestrator discards.
pub fn generate_water(grid: &mut Grid, spec: &RiverSpec) -> (Vec<Vec<usize>>, Vec<Vec<usize>>) {
    let mut rng = StdRng::seed_from_u64(spec.seed);

    let passes = lerp(8.0, 2.0, spec.bendiness).round() as usize;
    let cost = cost_field(grid, &mut rng, passes, spec.warp_amp, spec.warp_passes);

    let width = grid.width();
    let height = grid.height();

    let mut mains: Vec<Vec<usize>> = Vec::new();
    for i in 0..spec.count {
        let entry_col = (width * (i + 1) / (spec.count + 1)).clamp(1, width - 2);
        let entry = entry_col; // row 0 → index = col
        let mut rrng = StdRng::seed_from_u64(spec.seed ^ (i as u64 + 1));
        let drift_i = (spec.drift
            * (1.0 + rrng.random_range(-spec.drift_spread..spec.drift_spread)))
        .max(0.001);
        let bendiness_i = (spec.bendiness
            + rrng.random_range(-spec.bendiness_spread..spec.bendiness_spread))
        .clamp(0.0, 1.0);
        let penalty_i = lerp(PEN_HIGH as f32, PEN_LOW as f32, bendiness_i) as u32;

        // Goal: confluence cell on parent centerline, or normal bottom-edge exit.
        let parent_idx = spec.confluence_pairs.iter().find_map(|&(child, parent)| {
            if child == i { Some(parent) } else { None }
        });

        let goal = if let Some(parent) = parent_idx {
            // Goal = lower 40% of parent centerline (mirrors trib confluence selection).
            let parent_cl = &mains[parent];
            let lower_cutoff = height * 60 / 100;
            let lower_cells: Vec<usize> = parent_cl
                .iter()
                .copied()
                .filter(|&c| c / width > lower_cutoff)
                .collect();
            if lower_cells.is_empty() {
                // Fallback: any cell on the parent centerline.
                let idx = rrng.random_range(0..parent_cl.len());
                parent_cl[idx]
            } else {
                let idx = rrng.random_range(0..lower_cells.len());
                lower_cells[idx]
            }
        } else {
            let exit_col = ((entry_col as f32 + drift_i * height as f32).round() as isize)
                .clamp(0, width as isize - 1) as usize;
            (height - 1) * width + exit_col
        };

        let cl = carve(grid, &cost, entry, goal, Some((&spec.heading, penalty_i)));
        let mut riffle_rng = StdRng::seed_from_u64(spec.seed ^ ((i as u64 + 1) << 16));
        let profile = field::normalize(&field::value_noise(cl.len(), 1, spec.riffle_passes, &mut riffle_rng));
        stamp_main_channel(grid, &cl, &profile, spec);
        mains.push(cl);
    }

    features::place_oxbows(grid, &cost, &mut rng, spec);
    lake::generate_lakes(grid, &cost, spec);
    let tribs = features::carve_tributaries(grid, &cost, &mut rng, spec, &mains);

    tag_shallows_as_fords(grid, spec.riffle_ford_threshold);

    prox::compute_water_prox(grid, spec.water_reach, spec.lake_reach);

    (mains, tribs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::Grid;

    #[test]
    fn generation_is_deterministic() {
        let spec = RiverSpec::default();
        let mut g1 = Grid::new(128, 96);
        let mut g2 = Grid::new(128, 96);
        generate_water(&mut g1, &spec);
        generate_water(&mut g2, &spec);
        let w1: Vec<f32> = (0..g1.len()).map(|i| g1.water(i)).collect();
        let w2: Vec<f32> = (0..g2.len()).map(|i| g2.water(i)).collect();
        assert_eq!(w1, w2, "same seed must produce identical water fields");
    }

    #[test]
    fn fords_carry_low_water() {
        let spec = RiverSpec::default();
        let mut g = Grid::new(128, 96);
        generate_water(&mut g, &spec);
        for i in 0..g.len() {
            if g.is_ford(i) {
                let w = g.water(i);
                assert!(
                    w > 0.0 && w <= spec.riffle_ford_threshold + f32::EPSILON,
                    "ford cell {i} has water {w} outside (0, {}]",
                    spec.riffle_ford_threshold
                );
            }
        }
    }

    #[test]
    fn main_channel_has_deep_water() {
        let spec = RiverSpec::default();
        let mut g = Grid::new(128, 96);
        generate_water(&mut g, &spec);
        let deep_cells = (0..g.len()).filter(|&i| g.water(i) > 0.9).count();
        assert!(deep_cells > 0, "expected deep main-channel core cells");
    }

    #[test]
    fn water_levels_span_expected_range() {
        let spec = RiverSpec::default();
        let mut g = Grid::new(128, 96);
        generate_water(&mut g, &spec);
        let max_water = (0..g.len()).map(|i| g.water(i)).fold(0.0f32, f32::max);
        let has_shallow = (0..g.len()).any(|i| g.water(i) > 0.0 && g.water(i) < 0.5);
        assert!(max_water > 0.9, "expected deep main-channel water, got max={max_water}");
        assert!(has_shallow, "expected shallow tributary/ford cells");
    }

    #[test]
    fn rivers_flow_top_to_bottom() {
        let spec = RiverSpec::default();
        let width = 128usize;
        let height = 96usize;
        let mut g = Grid::new(width, height);
        let (mains, _) = generate_water(&mut g, &spec);

        assert_eq!(mains.len(), spec.count, "should carve {} main channels", spec.count);

        for (i, cl) in mains.iter().enumerate() {
            assert!(!cl.is_empty(), "main centerline {i} is empty");

            // carve() path: cl[0] = goal/exit, cl.last() = start/entry.
            let entry_idx = *cl.last().unwrap();
            let exit_idx = cl[0];

            let entry_row = entry_idx / width;
            let exit_row = exit_idx / width;

            assert_eq!(entry_row, 0, "main {i} entry must be row 0, got {entry_row}");

            let is_child = spec.confluence_pairs.iter().any(|&(child, _)| child == i);
            if is_child {
                // Child main: exit cell must lie on the parent's centerline.
                let parent = spec
                    .confluence_pairs
                    .iter()
                    .find_map(|&(child, parent)| if child == i { Some(parent) } else { None })
                    .unwrap();
                let parent_cells: std::collections::HashSet<usize> =
                    mains[parent].iter().copied().collect();
                assert!(
                    parent_cells.contains(&exit_idx),
                    "child main {i} exit cell {exit_idx} (row {exit_row}) is not on parent {parent} centerline"
                );
            } else {
                // Non-child main: must exit at bottom row with rightward drift.
                assert_eq!(
                    exit_row,
                    height - 1,
                    "main {i} exit must be row {}, got {exit_row}",
                    height - 1
                );

                let entry_col = entry_idx % width;
                let exit_col = exit_idx % width;
                assert!(
                    exit_col > entry_col,
                    "main {i} should drift rightward: entry_col={entry_col}, exit_col={exit_col}"
                );
            }
        }
    }

    #[test]
    fn confluence_child_joins_parent() {
        let spec = RiverSpec::default();
        let width = 128usize;
        let height = 96usize;
        let mut g = Grid::new(width, height);
        let (mains, _) = generate_water(&mut g, &spec);

        for &(child, parent) in &spec.confluence_pairs {
            let child_cl = &mains[child];
            assert!(!child_cl.is_empty(), "child {child} centerline is empty");

            let confluence = child_cl[0]; // cl[0] = carve goal = confluence on parent
            let parent_cells: std::collections::HashSet<usize> =
                mains[parent].iter().copied().collect();
            assert!(
                parent_cells.contains(&confluence),
                "child {child} exit cell {confluence} is not on parent {parent} centerline"
            );

            let row = confluence / width;
            let lower_cutoff = height * 60 / 100;
            assert!(
                row > lower_cutoff,
                "confluence at row {row} is not in lower portion (>{lower_cutoff} expected)"
            );
        }
    }

    #[test]
    fn tributaries_join_a_main_channel() {
        let spec = RiverSpec::default();
        let mut g = Grid::new(128, 96);
        let (mains, tribs) = generate_water(&mut g, &spec);

        assert!(!tribs.is_empty(), "expected at least one tributary to be carved");

        let main_cells: std::collections::HashSet<usize> =
            mains.iter().flatten().copied().collect();

        let max_len = 4 * spec.trib_length as usize;
        for (i, tcl) in tribs.iter().enumerate() {
            assert!(!tcl.is_empty(), "tributary {i} centerline is empty");

            let confluence = tcl[0]; // cl[0] = carve goal = confluence on main
            assert!(
                main_cells.contains(&confluence),
                "tributary {i} confluence cell {confluence} is not on any main centerline"
            );

            assert!(
                tcl.len() <= max_len,
                "tributary {i} is too long: {} cells (max {max_len}); long-stream bug detected",
                tcl.len()
            );
        }
    }

    #[test]
    fn per_river_params_differ() {
        let spec = RiverSpec {
            drift_spread: 0.5,
            bendiness_spread: 0.5,
            ..RiverSpec::default()
        };
        let width = 128usize;
        let height = 96usize;
        let mut g = Grid::new(width, height);
        let (mains, _) = generate_water(&mut g, &spec);

        assert!(mains.len() >= 2, "need at least 2 rivers to compare");

        let drifts: Vec<f32> = mains
            .iter()
            .map(|cl| {
                let entry_col = (*cl.last().unwrap() % width) as f32;
                let exit_col = (cl[0] % width) as f32;
                (exit_col - entry_col) / height as f32
            })
            .collect();

        let all_equal = drifts.windows(2).all(|w| (w[0] - w[1]).abs() < 0.01);
        assert!(
            !all_equal,
            "expected rivers to show per-river drift variation with spread=0.5, got: {:?}",
            drifts
        );
    }

    #[test]
    fn lakes_are_seeded() {
        let spec_with = RiverSpec::default();
        let spec_none = RiverSpec {
            big_lake_count: 0,
            minor_lake_count: 0,
            ..RiverSpec::default()
        };

        let mut g_with = Grid::new(128, 96);
        let mut g_none = Grid::new(128, 96);
        generate_water(&mut g_with, &spec_with);
        generate_water(&mut g_none, &spec_none);

        let deep_with = (0..g_with.len()).filter(|&i| g_with.water(i) >= 0.9).count();
        let deep_none = (0..g_none.len()).filter(|&i| g_none.water(i) >= 0.9).count();

        assert!(
            deep_with > deep_none,
            "expected lakes to add deep water cells: with_lakes={deep_with}, no_lakes={deep_none}"
        );
    }

    #[test]
    fn river_has_riffles_and_pools() {
        let spec = RiverSpec::default();
        let mut g = Grid::new(128, 96);
        let (mains, _) = generate_water(&mut g, &spec);

        let all_main_cells: std::collections::HashSet<usize> =
            mains.iter().flatten().copied().collect();

        const EPS: f32 = 0.05;
        let has_riffle = all_main_cells
            .iter()
            .any(|&i| g.water(i) <= spec.riffle_depth + EPS);
        let has_pool = all_main_cells.iter().any(|&i| g.water(i) >= 0.9);
        let has_ford = (0..g.len()).any(|i| g.is_ford(i));

        assert!(has_riffle, "expected at least one riffle-shallow cell on a main channel");
        assert!(has_pool, "expected at least one pool-deep cell on a main channel");
        assert!(has_ford, "expected at least one ford-tagged cell");
    }
}
