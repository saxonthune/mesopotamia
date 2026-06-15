# mesopotamia — design & modeling reference

A hobby project to make **artistic game-engine visualizations** of debt-bubble and
ecological-flow dynamics. This doc is the build reference: the conceptual premise, then the
buildable models with their actual state variables and update rules.

Sourced from a verified `/deep-research` pass (27 sources, 22 claims confirmed 3-vote
adversarial). Items tagged *verified* survived that vetting; the two refuted formulas are
called out so you don't build them.

---

## The premise (one engine, two skins)

**Debt-cascade models and ecological-flow models share one mathematical spine:** a
*weighted directed graph + an iterative update rule + a spectral/bifurcation tipping point.*
So **one simulation engine renders both** — swap the liabilities matrix `L` for an
ecological flow matrix `F` and the same cascade + early-warning machinery applies. Financial
contagion and ecosystem collapse become two skins on the same substrate.

⚠ **This is a rhyme, not a theorem.** Verification *refuted* (1-2) the claim that
critical-slowing-down warning signals are generic enough to license transferring ecological
collapse models directly onto debt dynamics. The shared tipping-point math is real *within*
each domain; using one visual substrate for both is artistically defensible but is a
*constructed structural analogy*, not a proven scientific equivalence. Correct register for
an art project — just don't mistake the rhyme for proof.

### The motivating idea (from the seeding dialogue)
- Debt binds economic pathways into "canals." When a bond doesn't blow up, its shape gets
  **etched** into the network (path dependence); when it does, members break free and new
  bonds form (a regime shift).
- The network's "mass" is **throughput, not stock** — social-metabolic flow, whose growth
  rate is set by net available energy. A bubble = overshoot of carrying capacity; the "pop"
  dissipates throughput entropically (idled capital, scrapped materials).
- The visual payoff: **ascendency vs. resilience** (efficiency vs. slack). A network that
  maximizes efficiency becomes *brittle* — too organized to absorb a shock. That is the
  picture of "etched canals → bubble."

---

## MODEL 1 — DebtRank (build this first)

The most tractable model. **State = one N×N matrix + three N-vectors:**

| symbol | meaning |
|---|---|
| `L` (= `A`) | interbank liabilities matrix — who owes whom; your weighted directed edges |
| `Ae` | external assets per bank (N-vector) |
| `Le` | external liabilities per bank (N-vector) |
| `equityBeforeShock` | pre-shock equity, for normalization (N-vector) |

**Update loop** (verbatim from Barucca's canonical `debtrank.m`, iterate to convergence):
```
valuationVector = max(equity, 0) ./ equityBeforeShock
equity          = Ae - Le + A*(valuationVector) - l     # l = interbank liabilities owed out
```
It's iterated balance-sheet accounting — a "microscopic foundation," not an ad-hoc cascade
rule.

**On screen:** nodes = banks sized by equity; edges = directed loans weighted by amount; a
shock animates equity erosion propagating debtor→creditor each round until it settles or a
bank defaults.

**The "pop" = a computable spectral threshold.** System is stable iff:
```
λ_max · e^α < 1          # λ_max = largest eigenvalue of the leverage matrix
```
Put a leverage/α dial + a live λ_max readout on screen and *watch* the network cross from
damping to amplifying a shock.

⚠ **Do NOT build these two refuted non-linear forms:**
- `h_i(t+1) = h_i(t) + (1−h_i)·Σ Λ_ij·p_D` — refuted 1-2
- `p_D = 1 − exp(−α·h)` — refuted 0-3

Build the **linear** Barucca loop above + apply the λ_max criterion separately.

- Code: `github.com/paolobarucca/debtrank` (`debtrank.m`)
- Paper: Bardoscia, Battiston, Caccioli & Caldarelli (2015), PLOS ONE, *DebtRank: A
  Microscopic Foundation for Shock Propagation*. *verified.*
- Spectral criterion: Bardoscia, Caccioli & Caldarelli (2016), PLOS ONE. *verified.*

---

## MODEL 2 — Generic cascade engine (one engine, many phenomena)

Collapses financial default, social/voter, epidemic, and fiber-bundle failure into **one
state variable: net fragility** (= fragility − failure threshold).

- A node **fails** when net fragility > 0.
- A failure **raises neighbors' fragility**, possibly triggering a cascade.
- **Systemic risk = one macroscopic number `X*`** (final fraction of failed nodes).
- Sweep initial conditions → a **phase diagram** of `X*`.

**On screen:** per-node fragility coloring during a cascade; a heat/phase diagram of `X*`
vs. initial-condition sliders. Same author lineage as DebtRank (Battiston) — coherent next
step after Model 1.

- Lorenz, Battiston & Schweitzer (2009), *Systemic Risk in a Unifying Framework for
  Cascading Processes on Networks*, arXiv:0907.5325. *verified.*

---

## MODEL 3 — Ulanowicz ascendency (the ecological skin)

Runs over the same weighted directed graph as Model 1.

**Ascendency** = one scalar fusing a flow network's size/growth with its organization:
```
A = TST × AMI                                    # Total System Throughput × Avg Mutual Information
A = Σ_ij  Tij · log( Tij·T.. / (Ti.·T.j) )       # T = flow matrix; T.. = grand total
```

**The decomposition that drives the bubble visual:**
```
A + R = C       # Development Capacity C = TST × H   (H = total flow diversity)
                # Ascendency A = rigid efficiency / organized order
                # Resilience R = C − A = slack / flexible reserve
```
Maximizing ascendency → brittleness. Animate the A-vs-R trade-off as sectors grow or get
shocked.

**Transfers directly to economies:** an **input–output table IS an ENA flow network**
(sectors = nodes, currency flows = directed edges). Demonstrated on Beijing's economy
(6 sectors, 1985–2010). This is the literal ecology↔economy crossover.

**State variables (ENA, mass-balanced):**

| symbol | meaning |
|---|---|
| `A` (adjacency) | `a_ij = 1` if flow goes `j → i`, else 0 — topology |
| `F` (flow matrix) | weighted flows between compartments |
| `x` (storage) | biomass/standing stock per compartment |
| `z` (inputs), `Ty` (outputs) | boundary flows |

Conservation at every compartment: `C = P + R + E` (consumption = production + respiration +
egestion). Enforce this; flag any unbalanced node.

**Hobbyist-scale:** published models run **3–60 compartments** (most aggregated = producers /
detritus / consumers). Well within a game engine's real-time budget.

**On screen:** compartments as nodes sized by storage `x`; weighted flow edges from `F`; live
ascendency / TST / AMI readout; a conservation check that flags unbalanced nodes.

- Ulanowicz (1980+), *Ecology, the Ascendent Perspective*. *verified.*
- Huang & Ulanowicz (2014), PLOS ONE, *Ecological Network Analysis for Economic Systems*.
  *verified.*
- Fath, Scharler, Ulanowicz & Hannon (2007), *Ecological network analysis: network
  construction* (the methods recipe). *verified (node-count 2-1).*

---

## MODEL 4 — The tipping-point overlay (works on BOTH families)

Near a **fold bifurcation** the dominant eigenvalue → 0, producing three generic
**early-warning signals**, all computable from a simulated time series:
- slower recovery from perturbations
- rising **lag-1 autocorrelation**
- rising **variance**

**On screen:** a rolling-window plot of variance + autocorrelation that visibly "flickers"
before the crash — droppable over the debt network *or* the ecosystem.

⚠ The *mechanism* for a simulated fold bifurcation is solid (*verified*), but the claim that
these signals are generic enough to transfer ecological models onto debt was **refuted
(1-2)** — keep it as artistic overlay, not proof.

**Companion — the ball-in-a-landscape picture:** a stability well that flattens as
resilience erodes, with hysteresis as a control parameter is swept up then down. The cleanest
low-dimensional overshoot→collapse visual.

- Scheffer et al. (2009), *Nature*, *Early-warning signals for critical transitions*.
  *verified.*
- Scheffer, Carpenter, Foley, Folke & Walker (2001), *Nature*, *Catastrophic shifts in
  ecosystems*. *verified.*

---

## Orientation & forks

- **Read first to map the subfield:** Caccioli, Barucca & Kobayashi (2017), *Network models
  of financial systemic risk: A review*, arXiv:1710.11512. Cleanly separates the two
  contagion channels to render: bilateral loans (edges) vs. overlapping portfolios
  (shared-asset hyperedges). *verified.*
- **NetLogo standard library forks:** *Wolf–Sheep Predation* (Lotka–Volterra
  overshoot-and-collapse) and *Sugarscape* (Epstein–Axtell emergent macro-from-micro).
- **Artistic-interactive register reference:** Nicky Case explorables (ncase.me).

---

## Build order

1. **DebtRank on a small toy interbank network** (Model 1). Cheapest state, legible cascade,
   live λ_max dial to *watch* the tipping point. **Start here.**
2. Add the **early-warning overlay** (Model 4) — variance + autocorrelation rolling plot.
3. **Reskin to ecology:** swap `L` → ENA flow matrix `F`, add the ascendency/resilience
   readout (Model 3). This is the "one substrate, two phenomena" payoff — minimal new code.
4. Generalize into the **net-fragility cascade engine** (Model 2) if you want financial /
   social / epidemic skins on one core.

## Open questions (not yet resolved by the research)
- Smallest realistic seed: toy interbank network vs. a published exposure matrix.
- Engine: does Godot's GraphEdit/2D suffice, or is custom rendering needed for live
  eigenvalue/variance overlays?
- Unsourced-but-wanted: clean low-dimensional Lotka–Volterra / carrying-capacity formulations,
  MuSIASEM metabolic accounting, the Santa Fe / Sugarscape ABM toolkit's seminal paper.

---

*Conceptual / philosophical reading that motivates the art (process + dialectical-materialist
constellation: self-organization, autopoiesis, morphogenesis, metabolic rift) lives in the
study-sessions session artifact `2026-06-11-debt-ecology-modeling-reading-list.md` — not
duplicated here.*
