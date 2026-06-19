use bevy::prelude::*;
use std::collections::HashMap;

use super::movement::Decision;

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
    /// Exponentially-weighted moving average of per-tick forage intake. Updated
    /// in `graze`; read by `herd_move` to compute the patch-leaving gate.
    pub intake_rate: f32,
}

/// The decision record written by `herd_move` each tick. Present on every elk;
/// overwritten in place to avoid per-tick archetype moves.
#[derive(Component)]
pub struct LastDecision(pub Decision);

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
    pub spawned: u32, // total elk created in this cohort's wave
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
#[derive(Resource, Clone)]
pub struct ElkParams {
    pub separation: f32,
    pub cohesion: f32,
    pub grass: f32,     // attraction up the grass gradient
    pub social: f32,    // attraction toward elk seen grazing (local enhancement)
    pub migration: f32, // weight of the far-edge fallback pull
    pub quiet: f32,     // migration-residual crossover: natural_strength at half migration weight
    pub cross: f32,     // hunger-scaled weight on the ford-toward-greener-far-bank incentive
    pub sep_radius: f32,
    pub coh_radius: f32,
    pub grass_radius: f32,
    pub social_radius: f32, // how far a grazing elk is noticed — longer than grass
    pub temperature: f32,   // step randomness; higher = more diffuse
    pub bite: f32,        // max grass eaten per graze (capped by what sits above the floor)
    /// Weight on the graze candidate's value, in desire-magnitude units; higher = elk pause to feed more readily.
    pub dwell: f32,
    pub graze_yield: f32, // energy per unit of grass actually consumed (proportional intake)
    pub graze_floor: f32, // giving-up density as a fraction of capacity; grass below isn't worth biting
    pub energy_drain: f32, // energy lost per tick; reaching 0 starves the elk
    pub mig_growth: f32,   // per-tick growth of migration pressure (0 → 1 ramp)
    pub water_cost: f32,     // step penalty for entering water — fording is costly
    pub ford_discount: f32,  // fraction of water_cost paid on a ford (0 → free, 1 → full cost)
    pub swim_drain: f32,     // energy drained when entering deep non-ford water
    pub shrub_bite: f32,    // shrubs stripped per graze — a big bite
    pub shrub_energy: f32,  // energy from a shrub bite — concentrated forage
    /// EWMA smoothing factor for per-elk intake rate (0 → frozen, 1 → instantaneous).
    pub intake_smoothing: f32,
    /// Ratio of habitat mean below which the patch-leaving gate fully closes.
    pub giving_up: f32,
    /// Migration amplification when the gate is fully closed: factor = 1 + leave_boost.
    pub leave_boost: f32,
}

/// Latest-tick mean drive breakdown per pack slot, written by `herd_move` and
/// read by the UI (pie/graphs) and the balancing sweep. Index by `slot`.
#[derive(Resource, Default)]
pub struct DriveSamples {
    /// Mean `Drives` over the elk of each slot this tick (zeroed `DriveSample` for
    /// an empty slot). Length == PACK_COUNT.
    pub per_slot: Vec<DriveSample>,
}

/// Per-slot mean drive magnitudes for one tick.
#[derive(Default, Clone, Copy)]
pub struct DriveSample {
    pub sep: f32,
    pub coh: f32,
    pub grass: f32,
    pub social: f32,
    pub migration: f32,
    pub count: u32,
}

impl DriveSample {
    /// Fraction of total pull effort that is the migration force, in [0, 1]:
    /// `migration` over the sum of all five magnitudes. 0 when nothing pulls.
    #[allow(dead_code)] // consumed by ui-declarative-panels-graphs and balancing-param-sweep
    pub fn migration_share(&self) -> f32 {
        let sum = self.sep + self.coh + self.grass + self.social + self.migration;
        if sum > 1e-6 { self.migration / sum } else { 0.0 }
    }
}

/// When present, `herd_move` uses this persistent RNG for deterministic picks
/// instead of the thread RNG. The `SmallRng` state evolves across ticks so
/// consecutive ticks produce different random values. Absent in normal runs.
#[derive(Resource)]
pub struct ProbeSeed(rand::rngs::SmallRng);

impl ProbeSeed {
    pub fn new(seed: u64) -> Self {
        use rand::SeedableRng;
        Self(rand::rngs::SmallRng::seed_from_u64(seed))
    }

    pub fn rng(&mut self) -> &mut rand::rngs::SmallRng {
        &mut self.0
    }
}

/// Running habitat-average intake — the mean of all elk `intake_rate` values,
/// recomputed each tick in `graze`. `herd_move` reads this to scale the patch-
/// leaving gate: an elk with below-average local intake is nudged to leave.
#[derive(Resource, Default)]
pub struct HabitatIntake {
    pub mean: f32,
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
            cross: 1.0,
            sep_radius: 3.0,
            coh_radius: 9.0,
            grass_radius: 5.0,
            social_radius: 16.0,
            temperature: 0.6,
            // Retuned down from 0.5 so a committed graze depletes a patch over
            // several ticks rather than one — turning the pause into a visible
            // multi-tick dwell. Sustainability bound still holds: intrinsic · cap · graze_yield < drain.
            bite: 0.12,
            dwell: 1.5,
            // Energy is proportional to grass actually eaten. A full fresh bite
            // (bite · this ≈ 0.0042) pays roughly drain, building up only over a
            // multi-tick graze — the herd must commit to a patch and dwell.
            graze_yield: 0.035,
            // Giving-up density: leave 30% of each cell's capacity uneaten. Grass
            // below graze_floor · capacity isn't worth biting, which seeds regrowth
            // and makes thin patches not worth the elk's time.
            graze_floor: 0.3,
            energy_drain: 0.004,
            mig_growth: 0.0015,
            water_cost: 2.0,
            ford_discount: 0.1,
            swim_drain: 0.01,
            shrub_bite: 0.34,
            shrub_energy: 0.05,
            intake_smoothing: 0.05,
            giving_up: 0.6,
            leave_boost: 1.5,
        }
    }
}
