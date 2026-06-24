---
title: Driftscape
summary: A suite of terminal screensavers, each a drift field past a viewport — the simulation's flow-past-a-frame rendered as truecolor terminal art
tags: [milestone, tui, screensaver, drift, parallax]
deps: []
---

# Driftscape

A suite of terminal screensavers. Each one is the same core motion the simulation is built on — a field of things drifting past a fixed viewport — staged in the terminal instead of a Bevy window. Where the grazing and hotspot milestones render flow into a graphics window, Driftscape renders it into a grid of colored glyphs: continuous-space positions and velocities sampled onto half-block cells, in truecolor.

The suite shares one engine and swaps the scene. Each screensaver is a drift field with its own content and its own layering, but they all reuse the same pieces: a pure step that advances positions by `dt` and respawns whatever falls off an edge, parallax as layered fields moving at speeds set by depth, and a half-block compositor that turns a pixel buffer into terminal output.

## Contents

- **doc03.03.01 Goal** — the first screensaver: multicolored planets drifting past a spaceship window, near ones large and fast, far ones small and slow.
