---
title: Goal
summary: The first Driftscape screensaver — multicolored planets drift past a spaceship window in the terminal, sized and paced by depth so near planets sweep large and fast while far ones crawl small and dim
tags: [milestone, tui, parallax, procgen, rendering]
deps: []
---

# Goal

## The scene

The terminal is the window of a moving spaceship. The frame fills with deep space — a near-black field stippled with stars — and **planets drift across it**, entering from one edge and passing out the other. Each planet is a shaded sphere in its own color: a lit side, a terminator, and a dark side, drawn from a single point of light so the ball reads as round rather than flat.

The planets come at different depths. A **near** planet is large, bright, and sweeps across quickly; a **far** one is small, dim, and crawls. The stars are the farthest layer, barely moving. Depth sets size, speed, and brightness together, so the layers separate into parallax and the frame feels like motion through space rather than a flat scroll.

The scene runs forever. As a planet leaves the frame, another is already entering behind it at a fresh depth, color, and size, so the procession never repeats and never ends.

## How it renders

The terminal cell is twice as tall as it is wide, so planets drawn one-pixel-per-cell would be eggs. The renderer instead treats each cell as **two stacked pixels** and prints the upper-half block `▀`: the glyph's foreground color is the top pixel, its background color is the bottom. This doubles the vertical resolution and squares the pixels, so the spheres are round and every pixel carries its own truecolor.

## What it demonstrates

The milestone's first screensaver is reached when the scene runs on its own in a real terminal: stars and planets drifting past at depths that read as parallax, each planet a rounded multicolored sphere, the procession renewing itself endlessly until the viewer quits.
