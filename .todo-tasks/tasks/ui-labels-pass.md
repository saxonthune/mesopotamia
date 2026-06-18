# UI labels pass — label everything, expand abbreviations

## Motivation

Product rule: **lots of labels, outside the graphical (world) space, with progressive disclosure.** The panels under-label and use cryptic abbreviations; an earlier change over-corrected the drive panel to hover-only colour swatches, hiding the labels. The world view stays clean, but panels label freely, with tooltips as the *deeper* disclosure tier — not the only one.

## Do NOT

- Do NOT label or annotate the world/graphical space — overlays, gizmos, sprites stay label-free.
- Do NOT change model behaviour, metrics, sliders' *semantics*, or parameter values. This is a text/label pass only.
- Do NOT remove the hover tooltips — they remain as the deeper tier beneath the visible labels.
- Keep the verification gate at `just test-fast` only.

## Plan

A pass over the egui panels in `src/ui.rs` (and any panel-adjacent code):

### 1. Restore drive labels

In `render_pie` / the drive panels, show a visible text label beside each colour swatch (Separation, Cohesion, Forage, Grazing cue, Migration), keeping the existing `DRIVE_EXPLAIN` hover text as the deeper tier. Use the `DRIVE_EXPLAIN`/`DRIVE_COLORS` ordering already in place.

### 2. Expand every abbreviation/acronym

Sweep all panel text and expand abbreviations to full words: e.g. "mig share" → "migration share", any "sep/coh/soc" → "separation/cohesion/social", and any other shortenings in slider labels, graph legends/series names, herd cards (e.g. clarify "slot", "peak", "gone"), and the decision panel. Series names in `PlotSpec` (e.g. `"mig share"`, `"forage/elk"`, `"regrowth÷drain"`) get readable full-word labels.

### 3. Add labels where panels rely on position/colour alone

Add a short heading/caption to any panel section that currently depends on layout or colour to be understood (the step-arrow row in the decision panel, the abundance graph series, etc.).

### 4. Label the new sliders clearly

Give clear, full-word labels to the patch-leaving sliders from the camping-fix phase (`intake_smoothing`, `giving_up`, `leave_boost`) and the ratio sliders from the ratio-sliders phase (the three ratio controls) — name what each *means*, not the field name.

## Files to Modify

- `src/ui.rs` — the label/abbreviation pass across `render_pie`, `behaviour_tab`, `grass_items`, `abundance_items`, `view_tab`, `herd_details`, `herd_card`, `elk_decision_panel`, and the graph series names.

## Verification

```bash
just test-fast
```

## Out of Scope

- Any model/metric/slider-semantics change.
- World-space annotation.

## Notes

- Honour the corrected rule (lots of labels outside the graphical space + progressive disclosure), not the earlier no-labels reading.

## Surface after this phase

- Panels carry visible full-word labels; abbreviations are expanded; the drive pie shows labels beside swatches with hover as the deeper tier.
- No model, metric, slider-semantics, or parameter-value changes; behaviour identical to the prior phase.
