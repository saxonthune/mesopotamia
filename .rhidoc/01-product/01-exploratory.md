---
title: Exploration
summary: Product vision — emergent agent-based simulation of ancient Mesopotamian debt and ecological cycles, the historical arc it dramatizes, and architecture/visualization directions
tags: [product, simulation, abm, history, vision]
deps: []
---

# Exploration

mesopotamia is an artistic, agent-based visualization of an ancient economy, where the macro-phenomena of debt bubbles and ecological overshoot — boom, crash, slow recovery — emerge from the local decisions of economic actors rather than from any top-down rule that detects a bubble or fires a crash.

The formal cascade and ecological-flow models that supply the underlying math live in `DESIGN.md` (the modeling reference). This doc holds the product vision: the emergent stance, the historical arc it dramatizes, and the architecture and visualization directions under exploration.

## The Emergent Stance

The simulation favors emergence over explicit control. There is no crash system, no global bubble detector, no equilibrium enforcer. Macro patterns are aggregate consequences of local rules.

The core entity is the **actor**, carrying:

- **Mass** — stored real value (output, wealth, biomass), a float ≥ 0.
- **Extraction rate** — mass pulled from the environment each tick; the only external input. Everything else is redistribution and debt.
- **Liabilities** — debts owed to other actors (creditor, principal, interest rate).

Each tick, actors extract mass, accrue and pay interest, and decide whether to borrow to invest. Borrowing raises an actor's extraction rate, which lets it service more debt and borrow again — the positive feedback that inflates a bubble. When extraction saturates or compound interest outruns real gains, an actor's mass falls below its interest due and it defaults: it loses its mass and its debts cancel. Creditors lose expected mass and may default in turn. The cascade is the natural consequence of sequential updates, not a scripted event.

Recovery follows. Survivors keep extracting, mass slowly re-accumulates, and cautious new lending forms a different network topology each run — hysteresis, never the same canals twice.

This realizes the motivating image from `DESIGN.md`: debt etches economic pathways into "canals" (path dependence); a network that maximizes efficiency (ascendency) grows brittle and snaps (the pop), then reorganizes.

## The Historical Frame

The artifact dramatizes the arc of an ancient river-valley economy from settlement to collapse.

**Beginning — emergent sedentism.** Cities are not placed by procedural generation; they settle through simulation. Hunter-gatherer bands form base camps around predictable wild resources; storage technology and communal structures make settlement permanent and draw new arrivals; climate "push" (post-glacial drying, megadrought) concentrates populations in rich river valleys, where water management lets density and organized labor rise. State formation is a cascade of emergent thresholds, observed rather than authored.

**Middle — palatial credit.** The economy runs on credit, not barter. Palaces and temples set prices, ration grain, and denominate debts in silver against a barley equivalent. Money is an administrative unit of account, meticulously tracked by scribes. Credit arises from the gap between planting and harvest.

**The debt cycle.** Agricultural debt is tied to a fixed harvest while commercial loans compound (often ~20% annually). Debts accumulate past what debtors can repay; land and labor concentrate in creditors' hands; social polarization rises. Kings periodically issue clean-slate decrees (*andurarum* / *misharum*) that cancel agricultural debts, free debt-slaves, and restore pledged land — resetting the spiral. The clean slate is the historical analogue of a deliberate, top-down intervention against an otherwise emergent crash.

**End — systemic shock.** The Late Bronze Age collapse (~1200 BCE) severs trade routes and starves the region of silver. Clean-slate decrees no longer suffice against combined internal debt and external shortage; authorities debase the currency (silver alloyed with up to 80% copper, then coated). The palatial credit order gives way to militarized Iron Age kingdoms and a bimetallic, coinage-based economy where iron weapons, not temple silver, are the source of authority.

## Architecture and Visualization

The architecture leans on an entity-component-system design (Bevy / Rust), with actors as entities, data in components (mass, extraction rate, liabilities, default history), and behavior in systems (extraction, interest payment, investment decision, loan matching). Crash and recovery are emergent, so no system bears their name.

Visualization directions under consideration:

- **Force-directed graph** — nodes sized by mass, edges weighted by debt principal; growth turns nodes green, defaults snap edges and fade nodes to grey. Shows network topology, the core of the instability hypothesis.
- **Particle flow** — mass as particles flowing along debt edges; overshoot crowds the edges, crash freezes the flow and emits drifting waste particles.
- **Macro readouts** — total mass, total debt principal, active edge count, mass inequality (Gini), and the ascendency-vs-resilience trade-off from the ecological skin in `DESIGN.md`.

## Open Questions

- **Emergent vs. formal.** Whether the on-screen dynamics are driven bottom-up by agent decisions (this exploration) or by the top-down matrix iteration of DebtRank and the ascendency models (`DESIGN.md`) — or whether the agent rules supply the micro-foundation that reproduces those formal macro-dynamics.
- **The clean slate as a lever.** Whether royal debt cancellation is an actor decision, a periodic rule, or an interactive control that lets a viewer intervene against the cascade.
- **Scope of the historical arc.** Whether settlement, the debt cycle, and the Bronze Age collapse are one continuous run or separate scenarios sharing the engine.
- **Environmental feedback.** Whether shared extraction degrades a common resource, closing an ecological overshoot loop on top of the debt loop.
- **Consolidation.** Whether actors merge into firms with varying productivity, changing network topology (fewer nodes, thicker edges).
