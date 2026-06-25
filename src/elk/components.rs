use bevy::prelude::*;
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
    /// Foraging mode. `false` = grazing in place; `true` = travelling — a committed
    /// directed dash to a fresh patch. Flipped by a Schmitt trigger on local patch
    /// richness (`next_forage_mode`); the hysteresis is what makes the herd roll
    /// forward as a leapfrogging wave instead of milling like a particle cloud.
    pub traveling: bool,
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
    /// Where the last wave spawned, in grid-cell coords `(col, row)`. The next
    /// wave random-walks from here so herds trail one another. `None` until the
    /// first wave seeds it.
    pub(crate) anchor: Option<Vec2>,
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
    pub momentum: f32,       // on land: reward for continuing last move's heading, resists backtracking
    pub momentum_water: f32, // in water: stronger persistence so a crossing elk commits to the far bank
    pub leave_frac: f32,     // graze→travel: leave a patch once local grass falls below this fraction of capacity
    pub travel_margin: f32,  // how much richer a reachable patch must be (in capacity-fraction) to be worth travelling to
    pub travel_focus: f32,   // softmax temperature multiplier while travelling (< 1 ⇒ sharper, more committed)
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
    /// Bias of the cohesion target toward packmates *ahead* (greater column). 0.0
    /// (default) ⇒ cohesion pulls to the plain slot centroid (today). Positive ⇒
    /// forward packmates weigh more, so the herd's cohesion centre drifts east and
    /// the blob elongates into a rolling column instead of clustering on its centre.
    pub cohesion_lead: f32,
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
            // Directional persistence. On land a gentle anti-backtrack nudge;
            // in water a strong commitment so an elk mid-river marches to the
            // far bank rather than oscillating. momentum_water is set above the
            // ford step penalty (water_cost · ford_discount = 0.2) so continuing
            // a started crossing reliably beats reversing out of it.
            momentum: 0.2,
            momentum_water: 1.0,
            // Foraging mode. An elk leaves a patch once its grass is drawn below
            // 40% of capacity *and* a patch at least 20 capacity-points richer is
            // within perception; it settles once nothing better is reachable. The
            // reachable-improvement test (not an absolute target) is what lets the
            // herd roll forward where richer ground exists yet stay and graze where
            // it doesn't — so it can never dash off thin terrain and starve.
            // leave_frac sits above graze_floor (0.3) so an elk leaves a thinning
            // patch before it bottoms out at the giving-up floor.
            leave_frac: 0.4,
            travel_margin: 0.2,
            // Travelling sharpens the pick to ~0.4× the grazing temperature, so a
            // dashing elk commits to its best direction instead of jittering.
            travel_focus: 0.4,
            sep_radius: 3.0,
            coh_radius: 9.0,
            grass_radius: 5.0,
            social_radius: 16.0,
            temperature: 0.6,
            // Slow chewing is the standard: a small bite means a patch depletes over
            // many ticks, so the grazing front advances slowly and the grass behind it
            // regrows into the gap — the herd rolls as a sticky glob and survives the
            // long march to the river instead of outrunning its own food. `bite_ratio`
            // (held at its default) and `graze_yield` adjust together so chew *rate*
            // changes but the per-bite economy does not.
            bite: 0.012,
            dwell: 1.5,
            // Placeholder only: `apply_ratios` overwrites this each tick from
            // `bite_ratio · energy_drain / bite` (≈ 0.42 at the defaults), so a full
            // bite pays `bite_ratio`× drain. The herd must commit to a patch and dwell.
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
            mig_growth: 0.0015,
            water_cost: 2.0,
            ford_discount: 0.1,
            swim_drain: 0.01,
            shrub_bite: 0.34,
            shrub_energy: 0.015,
            intake_smoothing: 0.05,
            giving_up: 0.6,
            leave_boost: 1.5,
            freshness_weight: 0.0,
            sightline_range: 0.0,
            sightline_weight: 0.0,
            cohesion_lead: 0.0,
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
