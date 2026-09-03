use bevy::prelude::*;
use rand::rngs::StdRng;
use rand::SeedableRng;
use std::collections::HashMap;

pub(crate) const MAX_COHORTS: usize = 64;

#[derive(Component)]
pub struct Elk {
    /// Grid cell — source of truth for all cell-based systems; always a real cell.
    pub cell: usize,
    /// Cell sliding from, for render interpolation; equals `cell` when settled.
    pub prev_cell: usize,
    /// Slide progress in [0, 1]; 1.0 = settled. Mid-step elk don't re-decide.
    pub move_t: f32,
    pub move_rate: f32,
    /// Herd-colour index; recycled across cohorts (unlike `code`, which is unique).
    pub slot: u8,
    /// Unique cohort identity; never recycled (unlike `slot`).
    pub code: u32,
    pub energy: f32,
    /// Poop timers; deposited where the elk stands when each expires.
    pub digesting: Vec<u32>,
    pub grazing: bool,
    /// Consecutive ticks at the far edge; despawns when it reaches `EDGE_TICKS`.
    pub at_edge: u32,
}

/// Drives the herd lifecycle: spaces spawns into waves and keeps the headcount
/// near a target that itself grows over time.
#[derive(Resource)]
pub struct Spawner {
    pub(crate) cooldown: u32,
    pub(crate) next_pack: u8,
    pub(crate) elapsed: u32,
    /// Last spawn origin; next wave random-walks from here. `None` until first wave.
    pub(crate) anchor: Option<Vec2>,
    /// Owned RNG so a fixed seed makes the whole spawn sequence reproducible.
    pub(crate) rng: StdRng,
}

impl Default for Spawner {
    fn default() -> Self {
        Spawner {
            cooldown: 0,
            next_pack: 0,
            elapsed: 0,
            anchor: None,
            rng: StdRng::seed_from_u64(rand::random()),
        }
    }
}

impl Spawner {
    // Only the native headless harness reseeds; on wasm that caller is compiled out.
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    pub(crate) fn reseed(&mut self, seed: u64) {
        self.rng = StdRng::seed_from_u64(seed);
    }
}

/// Backs the UI spawn panel: `count` is the slider/textbox value, `fire` is set by the button
/// and cleared by `manual_spawn` once it spawns. Used when auto-spawn is off (test maps).
#[derive(Resource)]
pub struct ManualSpawn {
    pub count: u32,
    pub fire: bool,
}

impl Default for ManualSpawn {
    fn default() -> Self {
        Self { count: 20, fire: false }
    }
}

/// Per-cohort lifetime record. Retained after death so the UI can show dead herds.
#[derive(Default, Clone)]
pub struct Cohort {
    pub slot: u8,
    pub alive: u32,
    pub energy_sum: f32, // average energy = energy_sum / alive
    pub peak: u32,
    pub deaths: u32,     // starvations
    pub departures: u32, // migrations off the far edge
    pub spawned: u32,
}

/// Cohort registry. `order` gives stable listing order and prune priority.
#[derive(Resource, Default)]
pub struct Herds {
    pub cohorts: HashMap<u32, Cohort>,
    pub order: Vec<u32>,
}

/// Food stripped per bite on *full* forage; a grazed-down cell yields proportionally less via the
/// Holling-II response in `metabolism::functional_response`. The elk slider tunes energy-per-food.
pub const BITE: f32 = 0.05;
/// Giving-up density: forage below `GRAZE_FLOOR × capacity` isn't worth biting. Low so cells graze
/// down to near-bare — kept above zero only so a worked-over patch stays faintly visible.
pub const GRAZE_FLOOR: f32 = 0.05;
/// Holling Type-II half-saturation (fullness units): roughly the fullness at which intake halves.
/// Lower ⇒ forage stays fast until heavily grazed ⇒ a sharper front (fast) vs back (slow) gradient.
pub const GRAZE_HALF_SAT: f32 = 0.2;
/// Per-tick metabolic drain — fixed; `feed_ratio` tunes intake against it, not this.
pub const ENERGY_DRAIN: f32 = 0.002;
/// Ticks locked chewing after a bite; paces intake to one bite per `CHEW_TICKS + 1`.
pub const CHEW_TICKS: u32 = 4;

/// Forage-perception and crossing tunables. Economy lives in `RatioControls`; steer weights in `HerdParams`.
#[derive(Resource, Clone)]
pub struct ElkParams {
    pub grass_radius: f32,
    pub graze_yield: f32, // shared grass+shrub yield; derived from feed_ratio by apply_ratios
    pub water_cost: f32,
    pub ford_discount: f32,  // fraction of water_cost paid on a ford (0 → free, 1 → full cost)
    pub swim_drain: f32,
    /// Weight on freshness vs standing crop; 0.0 = raw biomass gradient (default).
    pub freshness_weight: f32,
    /// Radius of the long-range, distance-discounted leave-target scan — how far a depleted elk
    /// can perceive the next patch across a void. 0.0 = off. Repurposed from the retired eastward
    /// sightline; the scan is omnidirectional now.
    pub sightline_range: f32,
    /// Per-cell value penalty on the long scan: a far cell's forage is discounted by
    /// `distance × sightline_discount`. This is the travel cost across bare ground — what makes the
    /// herd exploit local forage first and jump only once the local patch is stripped.
    pub sightline_discount: f32,
    /// Pull weight for the retired eastward `forage_sightline`; 0.0 = off. Inert in steering now.
    pub sightline_weight: f32,
    /// Look-across distance for crossing decisions; must exceed river width or the far bank is invisible.
    pub cross_peek: f32,
    /// Saturating decision cost for crossing (not linear; separate from swim energy drain).
    pub swim_reluctance: f32,
}

impl Default for ElkParams {
    fn default() -> Self {
        Self {
            grass_radius: 5.0,
            // Overwritten each tick by apply_ratios; placeholder only.
            graze_yield: 0.32,
            water_cost: 2.0,
            ford_discount: 0.1,
            swim_drain: 0.01,
            freshness_weight: 0.0,
            sightline_range: 24.0,
            sightline_discount: 0.012,
            sightline_weight: 0.0,
            // Above the widest worldgen channel (~19) so the far bank is always visible.
            cross_peek: 28.0,
            // Hungry herd facing fresh far-bank grass will commit; fed one won't.
            swim_reluctance: 0.6,
        }
    }
}
