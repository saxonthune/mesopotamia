use bevy::prelude::*;
use rand::rngs::StdRng;
use rand::SeedableRng;
use std::collections::HashMap;

pub(crate) const MAX_COHORTS: usize = 64;

#[derive(Component)]
pub struct Elk {
    /// Grid cell the elk occupies — the source of truth for every cell-based system
    /// (graze, score, events, edge detection). Movement is grid-locked: an elk only
    /// ever steps to one of its eight neighbours, so `cell` is always a real cell,
    /// never a sub-cell position.
    pub cell: usize,
    /// The cell the elk is sliding *from*, for render interpolation. Equals `cell`
    /// when settled.
    pub prev_cell: usize,
    /// Slide progress from `prev_cell` to `cell`, in [0, 1]. 1.0 = settled on `cell`;
    /// below 1.0 the elk is mid-step and committed (it does not re-decide). The
    /// renderer lerps `prev_cell → cell` by this, sub-sampled with the fixed-step
    /// overstep, so a step animates smoothly across frames rather than snapping.
    pub move_t: f32,
    /// Per-tick increment applied to `move_t` for the current slide — the step's
    /// animation speed in cells/tick. Zero when settled.
    pub move_rate: f32,
    /// Recycled pack *slot* (0..PACK_COUNT); the herd-colour index, recycled across
    /// cohorts (unlike `code`, which is unique per cohort).
    pub slot: u8,
    /// Stable cohort identity, shown as a 6-digit hex code. Unlike `slot`, this
    /// is unique per spawned cohort and never recycled.
    pub code: u32,
    /// Energy reserve in [0, 1]; drains each tick, replenished by grazing. Reaching
    /// zero starves the elk.
    pub energy: f32,
    /// Ticks remaining until each pending defecation. A meal eaten now lands
    /// here and is deposited as poop wherever the elk is when its timer expires.
    pub digesting: Vec<u32>,
    /// True on a tick the elk fed — drives the digestion timer and the grazing
    /// sprite tint.
    pub grazing: bool,
    /// Consecutive ticks spent at the far edge; once it crosses a threshold the
    /// elk has "left the map" and despawns.
    pub at_edge: u32,
}

/// Drives the herd lifecycle: spaces spawns into waves and keeps the headcount
/// near a target that itself grows over time.
#[derive(Resource)]
pub struct Spawner {
    /// Ticks until the next wave is allowed.
    pub(crate) cooldown: u32,
    /// Rolling pack id handed to the next cohort.
    pub(crate) next_pack: u8,
    /// Total ticks elapsed; drives the growth of the spawn rules.
    pub(crate) elapsed: u32,
    /// Where the last wave spawned, in grid-cell coords `(col, row)`. The next
    /// wave random-walks from here so herds trail one another. `None` until the
    /// first wave seeds it.
    pub(crate) anchor: Option<Vec2>,
    /// The seeded source for every wave's code, size jitter, and anchor walk.
    /// Owning it here (rather than drawing a fresh `rand::rng()` per wave) makes
    /// the whole spawn sequence a pure function of one seed, so a fixed-seed run
    /// replays identically. `Default` seeds from entropy — production stays fresh
    /// each launch; a test reseeds to a constant for reproducibility.
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
    /// Pin the spawn sequence to `seed` so a run replays identically. Used by the
    /// headless harness to make a worldgen diagnostic reproducible.
    pub(crate) fn reseed(&mut self, seed: u64) {
        self.rng = StdRng::seed_from_u64(seed);
    }
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

/// Every tunable that shapes how elk feed and perceive forage. The movement steer
/// itself is tuned by [`HerdParams`](super::herding::HerdParams); what remains here
/// is the metabolic economy (bite/yield/drain), the perception lengths the forage
/// scorers read, and the crossing costs. Radii are absolute perception lengths (an
/// elk's senses do not scale with the map).
#[derive(Resource, Clone)]
pub struct ElkParams {
    pub grass_radius: f32,
    pub bite: f32,        // max grass eaten per graze (capped by what sits above the floor)
    pub graze_yield: f32, // energy per unit of grass actually consumed (proportional intake)
    pub graze_floor: f32, // giving-up density as a fraction of capacity; grass below isn't worth biting
    pub energy_drain: f32, // energy lost per tick; reaching 0 starves the elk
    pub water_cost: f32,     // step penalty for entering water — fording is costly
    pub ford_discount: f32,  // fraction of water_cost paid on a ford (0 → free, 1 → full cost)
    pub swim_drain: f32,     // energy drained when entering deep non-ford water
    pub shrub_bite: f32,    // shrubs stripped per graze tick — small, so a shrub depletes over many ticks
    pub shrub_yield: f32,   // energy per unit of shrub actually consumed (proportional, like graze_yield)
    /// Weight of the per-cell freshness signal in the grass-gradient attractiveness.
    /// 0.0 (default) ⇒ the drive climbs raw forage — today's biomass gradient.
    /// Positive ⇒ fresher cells (recent regrowth) are pulled stronger; the herd
    /// steers toward the active green-up front rather than standing-crop peaks.
    pub freshness_weight: f32,
    /// How far east (cells along the migration axis) an elk looks for a clearly
    /// richer patch beyond its local `grass_radius` — the dry-land analogue of the
    /// across-river `forage_across` peek. 0.0 (default) ⇒ no long-range sight.
    pub sightline_range: f32,
    /// Weight on the forward pull produced by `forage_sightline`. 0.0 (default) ⇒
    /// off (identity: the grass drive is the local gradient only). Positive ⇒ an
    /// elk on depleted ground steers toward fresh forage it can see far ahead,
    /// the leapfrog primitive that carries the herd onto the next land segment.
    pub sightline_weight: f32,
    /// How far across a water span an elk looks for the far bank when deciding to
    /// cross — the crossing analogue of `grass_radius`. Set above the widest river so
    /// a hungry herd can *see* green far-bank forage instead of facing an opaque wall;
    /// below the river width the far bank is invisible and the river is uncrossable.
    pub cross_peek: f32,
    /// Saturating ceiling on the *decision* cost of a crossing, in forage-comparable
    /// units. A river crossing is one committed effort, so the cost a hungry elk
    /// weighs against far-bank forage saturates with width rather than growing
    /// linearly — a wide river deters but never becomes an infinite wall, which is
    /// what lets a herd ford on the natural forage drive instead of only the pull.
    /// (The per-tick swim *energy* drain is separate; this shapes only the choice.)
    pub swim_reluctance: f32,
}

impl Default for ElkParams {
    fn default() -> Self {
        Self {
            grass_radius: 5.0,
            // Slow chewing is the standard: a small bite means a patch depletes over
            // many ticks, so the grazing front advances slowly and the grass behind it
            // regrows into the gap — the herd rolls as a sticky glob and survives the
            // long march to the river instead of outrunning its own food. `bite_ratio`
            // (held at its default) and `graze_yield` adjust together so chew *rate*
            // changes but the per-bite economy does not.
            bite: 0.012,
            // Placeholder only: `apply_ratios` overwrites this each tick from
            // `bite_ratio · energy_drain / bite`, so a full bite pays `bite_ratio`× drain.
            graze_yield: 0.035,
            // Giving-up density: leave 30% of each cell's capacity uneaten. Grass
            // below graze_floor · capacity isn't worth biting, which seeds regrowth
            // and makes thin patches not worth the elk's time.
            graze_floor: 0.3,
            // Lowered to lengthen the metabolic timescale: an elk lives long enough to
            // ford the river and reach the far edge under the green wave, so survival is
            // the norm and `regrow_ratio` (scarcity) is what threatens it. Paired with
            // slow chewing above as the fixed standard physiology, not a player lever.
            energy_drain: 0.002,
            water_cost: 2.0,
            ford_discount: 0.1,
            swim_drain: 0.01,
            // Small bite so a shrub (seeded at full cap ~0.3–1.0) depletes over many
            // ticks; the elk dwells to strip it instead of eating it whole and running.
            shrub_bite: 0.05,
            shrub_yield: 0.1,
            freshness_weight: 0.0,
            sightline_range: 0.0,
            sightline_weight: 0.0,
            // Look across rivers up to 28 cells — above the widest worldgen channel
            // (~19) so the far bank is visible at every ford. The crossing decision
            // is gated by forage/hunger, not by whether the elk can perceive across.
            cross_peek: 28.0,
            // A crossing costs at most ~0.6 forage-units to decide on (saturating in
            // width); set near the high end of a fresh patch's worth so a hungry herd
            // facing green far-bank grass will commit, but a fed one won't wander in.
            swim_reluctance: 0.6,
        }
    }
}
