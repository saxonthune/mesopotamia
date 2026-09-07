//! `RiverSpec` — declarative authoring knobs for the watershed. Same spec → same world.

use rand::Rng;

pub(super) const RIVER_SEED: u64 = 0xBEDA;

// Directional penalty range: high = straight river, low = wanders freely.
pub(super) const PEN_HIGH: u32 = 5000;
pub(super) const PEN_LOW: u32 = 100;

pub enum Heading {
    Down,
}

pub struct RiverSpec {
    pub seed: u64,
    pub count: usize,
    pub heading: Heading,
    /// exit_col = entry_col + drift * height
    pub drift: f32,
    /// [0,1]: 0 = straight, 1 = wanders. Drives smoothing-pass count and penalty weight.
    pub bendiness: f32,
    /// Domain-warp displacement in cells; bends cost valleys into sinuous ones. 0 disables.
    pub warp_amp: f32,
    /// Smoothing passes for the warp displacement fields (wavelength).
    pub warp_passes: usize,
    pub trib_spacing: usize,
    pub trib_length: isize,
    pub core: isize,
    pub water_reach: u32,
    /// Shorter than `water_reach` so lake banks carry fewer grass rings than river banks.
    pub lake_reach: u32,
    pub trib_radius: isize,
    pub trib_depth: f32,
    pub oxbow_count: usize,
    pub oxbow_radius: isize,
    pub oxbow_depth: f32,
    pub riffle_depth: f32,
    pub riffle_radius: isize,
    pub pool_radius: isize,
    pub riffle_ford_threshold: f32,
    pub riffle_passes: usize,
    pub drift_spread: f32,
    pub bendiness_spread: f32,
    /// Blue-noise placed at interior cells (high dist-to-water) so they pool in open pockets.
    pub big_lake_count: usize,
    /// Metaball kernel radius; the summed field cut at `lake_iso` is the footprint and depth.
    pub big_lake_radius: isize,
    /// Repelled by big-lake centers so minors don't land on them.
    pub minor_lake_count: usize,
    pub minor_lake_radius: isize,
    /// Fraction of max dist-to-water a cell must clear to qualify as a big-lake site.
    pub big_lake_dist_frac: f32,
    /// 1 = round; 2+ offset kernels read as peanut/bulge.
    pub lake_lobes_max: usize,
    /// Max offset in cells for extra lobes — controls non-circularity.
    pub lake_offset: f32,
    /// Metaball iso-level; field below it = no water, above it = depth rising with field.
    pub lake_iso: f32,
    pub lake_samples: usize,
    /// `(child, parent)` pairs. Requires `parent < child` so the parent is carved first.
    pub confluence_pairs: Vec<(usize, usize)>,
}

// `parent < child` by construction so the parent is carved before the child seeks it.
pub fn random_confluences(count: usize, rng: &mut impl Rng) -> Vec<(usize, usize)> {
    if count < 2 {
        return Vec::new();
    }
    let child = rng.random_range(1..count);
    let parent = rng.random_range(0..child);
    vec![(child, parent)]
}

impl Default for RiverSpec {
    fn default() -> Self {
        Self {
            seed: RIVER_SEED,
            count: 3,
            heading: Heading::Down,
            drift: 0.25,
            bendiness: 0.5,
            warp_amp: 6.0,
            warp_passes: 12,
            trib_spacing: 24,
            trib_length: 12,
            core: 1,
            water_reach: 3,
            lake_reach: 2,
            trib_radius: 1,
            trib_depth: 0.35,
            oxbow_count: 2,
            oxbow_radius: 1,
            oxbow_depth: 0.3,
            riffle_depth: 0.3,
            riffle_radius: 4,
            pool_radius: 2,
            riffle_ford_threshold: 0.45,
            riffle_passes: 6,
            drift_spread: 0.2,
            bendiness_spread: 0.2,
            big_lake_count: 3,
            big_lake_radius: 5,
            minor_lake_count: 3,
            minor_lake_radius: 2,
            big_lake_dist_frac: 0.6,
            lake_lobes_max: 3,
            lake_offset: 4.0,
            lake_iso: 0.5,
            lake_samples: 16,
            confluence_pairs: vec![(1, 0)],
        }
    }
}
