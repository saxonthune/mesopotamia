//! Declarative authoring knobs for the watershed plus the flow-direction enum.
//! A `RiverSpec` (its `seed` field included) fully determines the water field;
//! same spec → same world.

/// Default watershed seed used by `RiverSpec::default()`. The full water field is
/// a pure function of the spec, so overriding `RiverSpec::seed` yields a fresh
/// river network from the same knobs.
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
    /// Seed for the watershed RNG. All channel, tributary, oxbow, and lake RNG
    /// derives from this, so the whole water field is a pure function of the spec.
    pub seed: u64,
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
    /// Cells between successive feeders along a main centerline.
    pub trib_spacing: usize,
    /// Lateral source offset / nominal feeder length in cells.
    pub trib_length: isize,
    /// Full-depth core radius (cells within the centerline that get max water).
    pub core: isize,
    /// BFS reach for the water-proximity carrying-capacity field.
    pub water_reach: u32,
    pub trib_radius: isize,
    pub trib_depth: f32,
    pub oxbow_count: usize,
    pub oxbow_radius: isize,
    pub oxbow_depth: f32,
    /// Water level at a full riffle (shallow end of the depth range).
    pub riffle_depth: f32,
    /// Bank radius at a riffle (wide).
    pub riffle_radius: isize,
    /// Bank radius at a pool (narrow).
    pub pool_radius: isize,
    /// Water cells at or below this level are tagged as fordable crossings.
    pub riffle_ford_threshold: f32,
    /// 1-D smoothing passes for the per-river riffle/pool profile.
    pub riffle_passes: usize,
    /// Fractional jitter applied to each river's drift: drift_i = drift * (1 ± spread).
    pub drift_spread: f32,
    /// Additive jitter on each river's bendiness (clamped to [0, 1]).
    pub bendiness_spread: f32,
    /// Number of big lakes, seated at blue-noise positions (Mitchell's best-candidate)
    /// among genuinely interior cells (high distance-to-water) so they pool in the
    /// centers of the open spaces the rivers leave.
    pub big_lake_count: usize,
    /// Base metaball kernel radius for each big lake, in cells. The first kernel sits
    /// at the lake center with this radius; the summed metaball field, cut at
    /// `lake_iso`, is the basin footprint and its depth.
    pub big_lake_radius: isize,
    /// Number of minor scattered lakes, seated at blue-noise positions among all
    /// lake sites (no interior requirement) and repelled by the big-lake centers.
    pub minor_lake_count: usize,
    /// Base metaball kernel radius for each minor lake, in cells — smaller than
    /// `big_lake_radius` so minors read as small pools.
    pub minor_lake_radius: isize,
    /// Fraction of the max distance-to-water a cell must clear to be a big-lake
    /// candidate (default 0.6): only cells deep in the open space between rivers
    /// qualify, so big lakes sit at the interior peaks.
    pub big_lake_dist_frac: f32,
    /// Max lobe count per lake (range `1..=lake_lobes_max`, biased low). 1 reads
    /// round; 2+ offset kernels read as a peanut or oblong bulge.
    pub lake_lobes_max: usize,
    /// Max kernel offset in cells for extra lobes. Small offsets bulge, medium ones
    /// read as a peanut — the knob that controls non-circularity.
    pub lake_offset: f32,
    /// Iso-level the summed metaball field is cut at: cells whose field is below it
    /// hold no lake water, above it take a depth rising with the field.
    pub lake_iso: f32,
    /// Best-candidate sample count K: each lake after the first is the farthest of
    /// `lake_samples` random candidates from the lakes already placed. Shared across
    /// both passes.
    pub lake_samples: usize,
    /// Confluence pairs `(child, parent)`: the child main merges into the parent
    /// instead of running to the bottom edge. Requires `parent < child` so the
    /// parent is already carved when the child is carved (mains carved in index order).
    pub confluence_pairs: Vec<(usize, usize)>,
}

impl Default for RiverSpec {
    fn default() -> Self {
        Self {
            seed: RIVER_SEED,
            count: 3,
            heading: Heading::Down,
            drift: 0.25,
            bendiness: 0.5,
            trib_spacing: 24,
            trib_length: 12,
            core: 1,
            water_reach: 8,
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
