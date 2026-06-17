//! Declarative authoring knobs for the watershed plus the flow-direction enum.
//! One `RiverSpec` together with `RIVER_SEED` fully determines the water field;
//! same pair → same world.

/// Seed for the watershed RNG. The full water field is a pure function of a
/// `RiverSpec` and this seed.
pub(super) const RIVER_SEED: u64 = 0xBEDA;

/// Directional cost constants: against-heading steps are penalised by this much
/// on top of the base noise cost (scale ~1–1001). Low bendiness → high penalty
/// (river runs straight); high bendiness → low penalty (river wanders freely).
pub(super) const PEN_HIGH: u32 = 5000;
pub(super) const PEN_LOW: u32 = 100;

/// Primary flow direction for the river. Only `Down` (top→bottom) is exercised;
/// the enum is kept minimal so a future `Right` variant is a small addition.
pub enum Heading {
    Down,
}

/// Declarative authoring knobs for the watershed. One `RiverSpec` plus the
/// `RIVER_SEED` fully determines the water field; same pair → same world.
///
/// Could become a Bevy `Resource` later to expose knobs to the editor UI.
pub struct RiverSpec {
    /// Number of main rivers (default 3 — evenly spaced for elk crossings).
    /// Entry columns are distributed across the width as width*(i+1)/(count+1),
    /// so spacing tracks the grid size instead of a fixed column period.
    pub count: usize,
    /// Primary flow direction (default `Down`: top-edge → bottom-edge).
    pub heading: Heading,
    /// Lateral lean: exit_col = entry_col + drift * height (default ~0.25).
    pub drift: f32,
    /// [0, 1] single knob: 0 = nearly straight / broad bends, 1 = wanders far.
    /// Drives both the smoothing-pass count (wavelength) and the directional-
    /// penalty weight — one intuitive control instead of two separate dials.
    pub bendiness: f32,
    /// Shallow feeders that branch off the main channels at a confluence.
    pub tributaries: usize,
    /// Main-channel raster radius (cells from centerline to bank).
    pub radius: isize,
    /// Full-depth core radius (≤ radius; cells within get max water).
    pub core: isize,
    /// BFS reach for the water-proximity carrying-capacity field.
    pub water_reach: u32,
    pub trib_radius: isize,
    pub trib_depth: f32,
    pub oxbow_count: usize,
    pub oxbow_radius: isize,
    pub oxbow_depth: f32,
    pub ford_spacing: usize,
    pub ford_depth: f32,
    /// Fractional jitter applied to each river's drift: drift_i = drift * (1 ± spread).
    pub drift_spread: f32,
    /// Additive jitter on each river's bendiness (clamped to [0, 1]).
    pub bendiness_spread: f32,
    /// Number of large standing-water lakes seeded at the deepest basins.
    pub lake_count: usize,
    /// Raster radius for each lake (cells from center to shore).
    pub lake_radius: isize,
    /// Full-depth core radius for each lake.
    pub lake_core: isize,
    /// Confluence pairs `(child, parent)`: the child main merges into the parent
    /// instead of running to the bottom edge. Requires `parent < child` so the
    /// parent is already carved when the child is carved (mains carved in index order).
    pub confluence_pairs: Vec<(usize, usize)>,
}

impl Default for RiverSpec {
    fn default() -> Self {
        Self {
            count: 3,
            heading: Heading::Down,
            drift: 0.25,
            bendiness: 0.5,
            tributaries: 2,
            radius: 3,
            core: 1,
            water_reach: 8,
            trib_radius: 1,
            trib_depth: 0.35,
            oxbow_count: 2,
            oxbow_radius: 1,
            oxbow_depth: 0.3,
            ford_spacing: 20,
            ford_depth: 0.3,
            drift_spread: 0.2,
            bendiness_spread: 0.2,
            lake_count: 3,
            lake_radius: 4,
            lake_core: 2,
            confluence_pairs: vec![(1, 0)],
        }
    }
}
