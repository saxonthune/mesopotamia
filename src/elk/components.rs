use bevy::prelude::*;
use std::collections::HashMap;

pub(crate) const MAX_COHORTS: usize = 64;

#[derive(Component)]
pub struct Elk {
    pub cell: usize,
    /// The cell the elk occupied before its last move. The simulation only ever
    /// reads `cell`; this exists purely so the renderer can slide the sprite from
    /// the old cell to the new one across a tick instead of snapping.
    pub prev_cell: usize,
    /// Recycled pack *slot* (0..PACK_COUNT); indexes `Packs` and `HerdStats`.
    pub slot: u8,
    /// Stable cohort identity, shown as a 6-digit hex code. Unlike `slot`, this
    /// is unique per spawned cohort and never recycled.
    pub code: u32,
    /// Drive signal in [0, 1]; drains each tick, replenished by grazing. The
    /// future decision AI reads this to decide what the elk is trying to do.
    pub energy: f32,
    /// Ticks remaining until each pending defecation. A meal eaten now lands
    /// here and is deposited as poop wherever the elk is when its timer expires.
    pub digesting: Vec<u32>,
    /// True on a tick the elk fed — the beacon other elk follow (social foraging).
    pub grazing: bool,
    /// Consecutive ticks spent at the far edge; once it crosses a threshold the
    /// elk has "left the map" and despawns.
    pub at_edge: u32,
}

/// Drives the herd lifecycle: spaces spawns into waves and keeps the headcount
/// near a target that itself grows over time.
#[derive(Resource, Default)]
pub struct Spawner {
    /// Ticks until the next wave is allowed.
    pub(crate) cooldown: u32,
    /// Rolling pack id handed to the next cohort.
    pub(crate) next_pack: u8,
    /// Total ticks elapsed; drives the growth of the spawn rules.
    pub(crate) elapsed: u32,
}

/// A spawned cohort's lifetime record, keyed by its hex `code`. Retained after
/// the cohort dies out so a panel can stay parked on a dead/migrated herd.
#[derive(Default, Clone)]
pub struct Cohort {
    pub slot: u8,
    pub alive: u32,
    pub energy_sum: f32, // average energy = energy_sum / alive
    pub peak: u32,       // largest the cohort ever was
    pub deaths: u32,     // starvations
    pub departures: u32, // migrations off the far edge
}

/// Registry of every (recent) cohort, keyed by hex code. `order` is spawn order,
/// used for stable dropdown listing and for pruning the oldest dead cohorts.
#[derive(Resource, Default)]
pub struct Herds {
    pub cohorts: HashMap<u32, Cohort>,
    pub order: Vec<u32>,
}

/// Per-pack state. Each pack migrates toward the far side under its own,
/// independently growing pressure, so packs depart on staggered clocks.
#[derive(Resource)]
pub struct Packs {
    /// Current migration pressure per pack (pushes +x toward the far edge).
    pub migration: Vec<f32>,
    /// Per-pack growth added to `migration` each tick.
    pub growth: Vec<f32>,
}

impl Packs {
    pub(crate) fn new() -> Self {
        let migration = vec![0.0; super::PACK_COUNT];
        // A unitless per-pack multiplier on the global migration growth rate;
        // reseeded per cohort at spawn. Staggers when each pack departs.
        let growth = (0..super::PACK_COUNT)
            .map(|i| 0.5 + i as f32 / super::PACK_COUNT as f32)
            .collect();
        Self { migration, growth }
    }
}

/// Every tunable that shapes how elk move and feed. Each movement drive is a
/// pure weight on a *normalized* direction, so the weights are real ratios that
/// hold their meaning across grid sizes and densities. Radii are absolute
/// perception lengths (an elk's senses do not scale with the map).
#[derive(Resource)]
pub struct ElkParams {
    pub separation: f32,
    pub cohesion: f32,
    pub grass: f32,     // attraction up the grass gradient
    pub social: f32,    // attraction toward elk seen grazing (local enhancement)
    pub migration: f32, // weight of the far-edge fallback pull
    pub quiet: f32,     // migration-residual crossover: natural_strength at half migration weight
    pub sep_radius: f32,
    pub coh_radius: f32,
    pub grass_radius: f32,
    pub social_radius: f32, // how far a grazing elk is noticed — longer than grass
    pub temperature: f32,   // step randomness; higher = more diffuse
    pub bite: f32,        // grass eaten per graze
    pub energy_per_bite: f32,
    pub energy_drain: f32, // energy lost per tick; reaching 0 starves the elk
    pub mig_growth: f32,   // per-tick growth of migration pressure (0 → 1 ramp)
    pub water_cost: f32,     // step penalty for entering water — fording is costly
    pub ford_discount: f32,  // fraction of water_cost paid on a ford (0 → free, 1 → full cost)
    pub swim_drain: f32,     // energy drained when entering deep non-ford water
    pub browse_bite: f32,    // browse stripped per graze — a big bite
    pub browse_energy: f32,  // energy from a browse bite — concentrated forage
}

impl Default for ElkParams {
    fn default() -> Self {
        Self {
            separation: 1.0,
            cohesion: 0.6,
            grass: 1.4,
            social: 0.8,
            // Raised from 0.35 to 0.7 so that at typical natural_strength ≈ 2.0 (all drives
            // active, well-fed herd) the effective pull is ~0.35 — matching the old constant.
            // quiet = 2.0 sets the crossover there; balancing-param-sweep tunes both properly.
            migration: 0.7,
            quiet: 2.0,
            sep_radius: 3.0,
            coh_radius: 9.0,
            grass_radius: 5.0,
            social_radius: 16.0,
            temperature: 0.6,
            bite: 0.5,
            // Sits a few × above the break-even grazing fraction (drain / this),
            // so a well-fed herd thrives but an overgrazed one starves — instead
            // of the old 0.2, which was ~50× break-even and pinned energy at the cap.
            energy_per_bite: 0.012,
            energy_drain: 0.004,
            mig_growth: 0.0015,
            water_cost: 2.0,
            ford_discount: 0.1,
            swim_drain: 0.01,
            browse_bite: 0.34,
            browse_energy: 0.05,
        }
    }
}
