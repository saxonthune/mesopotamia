# Grazers — Next Steps

Backlog for milestone 1. Each feature is another field over the grid, or a tweak to
the existing growth/eat systems. Updated only on request.

## Order

- **A. Elk eats grass** — binary `Grass → Empty` on the elk's cell. (in progress)
- **B. Grass height** — `Cell` carries a height value; eating subtracts, growth adds.
  Foundational; poop and the slider depend on it.
- **C. Poop field** — poop is a field (per-cell layer), not entities. Elk writes poop
  to its tile after eating.
- **D. Poop ↔ grass** — poop fertilizes: raises local growth rate / height. Needs B + C.
- **E. Growth-rate slider** — UI control feeding a growth-rate resource; can go slightly
  negative (grass decays). Needs a UI lib (likely bevy_egui).
- **F. River + water + migration** — research first. Procgen river → water field; grass
  growth reads water proximity; elk gain a migration urge rising with ticks. Last.

## Notes

- Grass, poop, water are all `Vec`s indexed the same way — hoist the index math onto
  `Grid` when the second field lands.
