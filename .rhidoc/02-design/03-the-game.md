---
title: The Game
summary: The grazers demo as light play — a three-stage arc from a broken herd, to a working migration the player tunes by hand, to an open score loop where the player cranks scarcity and refines a surviving model for the highest score
tags: [design, game, play, score, difficulty, tuning, arc]
deps: [doc02.01, doc03.01.08]
---

# The Game

The grazers demo is a tuning game. The simulation is the toy; the play is the act of
reading it (doc02.01) and adjusting the model's sliders until the herd does something
beautiful, then holding that beauty against rising scarcity. The experience is a
three-stage arc.

## Stage 1 — the broken herd

The player opens onto a model that does not work. Under the stock weights the herd
starves and veers in circles: it overcrowds the first stretch of map, eats it bare,
mills without direction, and dies. Nothing crosses the river. This is the **Default**
preset, and it is meant to look broken — the problem statement, posed in motion.

## Stage 2 — make it migrate

The player fixes it by tuning the model's behavioural weights — foraging strength,
dispersal, perception, the crossing appetite — until the herd feeds itself and rolls
across the map as a coherent wave. The decisive lever is the **natural eastward drive**:
the green wave, a travelling crest of fresh forage. A herd tuned to track the *freshest*
grass migrates with the crest, because following peak forage quality is itself the
journey east. Reaching this is the demo's core puzzle: the satisfaction of turning a
dying, aimless herd into a living migration by adjusting numbers and watching the system
answer.

The migration **pull** — a direct eastward force — exists as a crutch, not the intended
solution. It can drag a herd across without any model skill, so the score penalizes
leaning on it: a crossing earned by the natural drive is worth more than one bought with
the pull. The pull is training wheels; the green wave is riding.

## Stage 3 — the score loop

A working model opens the endgame. The **Survival Score** rates how far the herd gets,
gated and scaled by **difficulty** — the scarcity the player dials in by lowering forage
availability. A high score demands a hard world *and* a herd that still survives and
advances through it.

The score rewards **surviving**, not merely reaching far before dying. A crossing pays
full credit; a starvation pays its eastward progress *minus a death penalty*, so an elk
that dies even at the far bank scores well below one that crosses alive, and an elk that
dies early scores *negative* — a wipeout actively sinks the rating. A herd that marches
east and starves en masse therefore rates far below one that arrives intact. This is what
makes a high score mean what it should: most of the herd lived.

The loop is: crank the difficulty, watch the working model begin to fray under the
scarcity, make a small tuning adjustment to shore it up, and push the difficulty higher.
**Maxing the difficulty slider is the intended goal, not a degenerate exploit** — the
challenge is not whether to make the world harder but whether your *model* keeps working
when you do. Each increment is a small, legible tuning problem layered on the last, and
the high-water mark is the trophy: "your high was 168."

The pull penalty is what keeps this loop honest. Without it, the score collapses to
"max the pull, max the scarcity" and the model tuning stops mattering. With it, the only
way the score climbs is a model that genuinely surfs the green wave through famine — so
stage 3 is a continuation of stage 2's craft, not an escape from it.

## The shape of the whole

Three stages, one skill: reading the simulation and tuning it. Stage 1 shows the system
is real (it can fail). Stage 2 is the first win (make it migrate). Stage 3 is the open
loop (how hard a world can your migration survive). The presets name the waypoints —
Default is stage 1, Crossing is the stage-2 win, High score is a stage-3 target — and
the player's own slider adjustments are the play between them.
