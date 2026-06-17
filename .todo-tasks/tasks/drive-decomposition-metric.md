# Drive decomposition metric + residual migration (combine_drives)

## Motivation

The demo-1 goal (doc03.01.01, "The Balancing Goal") is to surface how much of a herd's
movement comes from the **natural drives** (separation, cohesion, grass-seeking, social
foraging) versus the **migration pull** (the "magic magnetic force" toward +x), and to make the
migration pull a genuine *fallback* that recedes as the naturals carry the herd. Today
`herd_move` (`src/elk/movement.rs:106-114`) computes the combined desire vector inline,
discards the per-drive components, and adds migration as an **unconditional constant** — even
though the code already *calls* it a fallback (UI "migration (fallback)", the `movement.rs`
doc comment "the migration fallback", `ElkParams` "far-edge fallback pull"). This task closes
that gap: it extracts the per-drive contributions into a pure metric AND makes migration a
residual that fills in only as the natural drives fall quiet.

This is the linchpin for two follow-on tasks (`balancing-param-sweep`, `ui-declarative-panels-graphs`),
which both read `combine_drives` / `migration_share`.

Follows the project coding patterns (doc02.01): the decision is extracted into pure free
functions pinned by metamorphic tests, exactly as `cross_desire` / `step_water_penalty` are.
Read doc02.01 and `.claude/skills/verification/SKILL.md` before starting.

## Do NOT

- **Do NOT keep migration as a constant additive term.** The whole point is the residual:
  migration is scaled by `migration_residual(natural_strength, quiet)`.
- **Do NOT** leave the share denominator as the resultant vector magnitude. The share is
  **sum of the five component magnitudes** (chosen so opposing drives that cancel still count).
- **Do NOT** add a `proptest` dependency. Concrete + metamorphic `#[test]`s only, in
  `movement.rs`'s test module, matching the `cross_desire` style directly above.
- **Do NOT** hard-code the crossover. `quiet` is a new `ElkParams` field with a slider, per the
  "tunables are params, not constants" rule (doc02.01).
- **Do NOT** silently break `tests/macro_sim.rs`. The residual weakens migration; if herds no
  longer reach the far edge, pick provisional `quiet` / `migration` defaults that keep the soak
  invariants green and document the chosen values (the `balancing-param-sweep` task tunes them
  properly later). See Plan step 5.
- **Do NOT** put `combine_drives` behind anything ECS — it takes plain `Vec2`/`f32`/`&ElkParams`
  and returns a struct. No `Res`, no `Grid`, no RNG.

## Plan

### 1. Pure crossover function (`src/elk/movement.rs`)

Add next to `cross_desire`:

```rust
/// Residual weight on the migration pull, in (0, 1]. Migration is a *fallback*:
/// it fills in only as the natural drives fall quiet. `quiet` is the crossover —
/// the `natural_strength` at which migration is at half weight. Pure and tiny so
/// the fade's shape is a one-line swap and the tests below still hold.
pub fn migration_residual(natural_strength: f32, quiet: f32) -> f32 {
    1.0 / (1.0 + natural_strength / quiet.max(1e-6))
}
```

### 2. Drives struct + combine_drives (`src/elk/movement.rs`)

```rust
/// The five weighted contribution vectors that sum into an elk's step desire,
/// kept separate so the migration-vs-natural split is legible and testable.
pub struct Drives {
    pub sep: Vec2,
    pub coh: Vec2,
    pub grass: Vec2,
    pub social: Vec2,
    pub migration: Vec2,
}

impl Drives {
    /// The combined desire vector — what `herd_move` steers by.
    pub fn total(&self) -> Vec2 { self.sep + self.coh + self.grass + self.social + self.migration }

    /// Fraction of total pull effort that is the migration ("magic") force, in
    /// [0, 1]: |migration| over the sum of all five component magnitudes. 0 when
    /// nothing pulls. Opposing drives that cancel still count toward the total.
    pub fn migration_share(&self) -> f32 {
        let sum = self.sep.length() + self.coh.length() + self.grass.length()
            + self.social.length() + self.migration.length();
        if sum > 1e-6 { self.migration.length() / sum } else { 0.0 }
    }
}

/// Combine the raw per-drive direction accumulators into weighted contributions.
/// `sep`/`coh`/`grass_dir`/`social` are the un-normalized accumulators built in
/// `herd_move`; `pressure` is the pack's migration pressure; `energy` is the elk's
/// energy (hunger sharpens the food drives). Migration is residual — it scales by
/// how quiet the naturals are.
pub fn combine_drives(
    sep: Vec2, coh: Vec2, grass_dir: Vec2, social: Vec2,
    params: &ElkParams, pressure: f32, energy: f32,
) -> Drives {
    let hunger = 1.0 - energy;
    let appetite = 0.25 + 0.75 * hunger;
    let sep = norm(sep) * params.separation;
    let coh = norm(coh) * params.cohesion;
    let grass = norm(grass_dir) * (params.grass * appetite);
    let social = norm(social) * (params.social * appetite);
    let natural_strength = sep.length() + coh.length() + grass.length() + social.length();
    let migration =
        Vec2::X * (params.migration * pressure * migration_residual(natural_strength, params.quiet));
    Drives { sep, coh, grass, social, migration }
}
```

`norm` and `ElkParams` are already in scope in `movement.rs`.

### 3. Refactor herd_move to call it (`src/elk/movement.rs:106-114`)

Replace the inline `let hunger = …; let appetite = …; let desire = …;` block with:

```rust
let drives = combine_drives(
    sep, coh, grass_dir, social, &params, packs.migration[slot as usize], elk.energy,
);
let desire = drives.total();
```

Everything downstream (`scores`, softmax, the step pick) is unchanged.

### 4. Add the `quiet` param (`src/elk/components.rs`) + slider (`src/ui.rs`)

- In `ElkParams` add `pub quiet: f32, // migration-residual crossover: natural_strength at half migration weight`.
- In `Default for ElkParams` add a provisional `quiet: 0.5` (a starting point the
  `balancing-param-sweep` task tunes). Adjust in step 5 if the soak test needs it.
- In `behaviour_tab` (`src/ui.rs:351`), next to the migration slider, add
  `slider(ui, &mut p.quiet, 0.05..=3.0, "migration crossover (quiet)");`

### 5. Keep the soak invariants green

Run `cargo test`. The residual reduces migration, so `tests/macro_sim.rs`'s
"herds reach the far edge" invariant is the one at risk. If it fails, raise the default
`migration` and/or `quiet` until herds still reach the edge band under the residual, and record
the chosen defaults in a code comment on the `ElkParams::default` fields. Do not loosen the
test's envelope to paper over a genuinely neutered migration.

### 6. Re-export for downstream (`src/elk/mod.rs`)

Add `Drives`, `combine_drives`, `migration_residual` to the `pub use movement::{…}` re-export
so the sweep and UI tasks reach them as `mesopotamia::elk::{Drives, combine_drives}`.

### 7. Tests (`src/elk/movement.rs` test module)

Concrete + metamorphic, no proptest. At least:

- `migration_residual`: `=1.0` at `natural_strength 0`; `≈0.5` at `natural_strength == quiet`;
  strictly decreasing in `natural_strength`; `→ ~0` for large `natural_strength`.
- `migration_share`: rises with `params.migration`; rises with `pressure`; **falls as the
  natural weights grow** (the residual makes stronger naturals show *less* magic force — this
  replaces the old "scaling naturals preserves share" relation, which only held for the
  discarded constant model); hungrier `energy` raises the grass+social share.
- A `Drives::total()` sanity case on hand-picked inputs.

For each metamorphic test, the input change that would break it must be nameable (doc02.01 /
verification skill) — assert directions, not magic numbers.

## Files to Modify

- `src/elk/movement.rs` — `migration_residual`, `Drives`, `combine_drives`; refactor `herd_move`; tests.
- `src/elk/components.rs` — `ElkParams.quiet` field + default.
- `src/ui.rs` — `quiet` slider in `behaviour_tab`.
- `src/elk/mod.rs` — re-export `Drives`, `combine_drives`, `migration_residual`.

## Verification

```sh
cargo build --bin mesopotamia
cargo build --bin demo1
cargo test
bash smoke.sh --no-run
```

## Out of Scope

- The UI pie chart / line graphs (`ui-declarative-panels-graphs`).
- The headless sweep / balancing harness (`balancing-param-sweep`).
- Tuning `quiet` / weights to an actual balanced set beyond keeping the soak test green.

## Notes

- The residual is a real behaviour change, deliberately folded into this task: extraction +
  model change together, pinned by residual-behaviour tests instead of a bit-identical
  equivalence test.
- `natural_strength` is computed once and used both for the residual factor and (implicitly,
  via the component magnitudes) the share denominator — keep them consistent (sum of magnitudes).

## Surface after this phase

- `mesopotamia::elk::Drives` — `{ sep, coh, grass, social, migration: Vec2 }` with
  `total() -> Vec2` and `migration_share() -> f32` (sum-of-magnitudes denominator, 0 when idle).
- `mesopotamia::elk::combine_drives(sep, coh, grass_dir, social, &ElkParams, pressure, energy) -> Drives`
  — pure; `total()` is the vector `herd_move` steers by.
- `mesopotamia::elk::migration_residual(natural_strength, quiet) -> f32` — pure crossover in (0,1].
- `ElkParams.quiet: f32` — the residual crossover, with a slider in `behaviour_tab`.
- Behaviour: migration is now a residual (scaled by `migration_residual`), NOT a constant term.
  `tests/macro_sim.rs` invariants still pass under the chosen defaults.
- Negative space: no graphs, no sweep harness, no per-tick history sampling yet — downstream
  tasks add those and may sample `migration_share` each tick.
