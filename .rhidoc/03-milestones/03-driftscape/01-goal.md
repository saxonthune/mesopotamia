---
title: Goal
summary: The first Driftscape screensaver — the terminal is a spaceship flying forward through space, planets emerging small at the vanishing point, growing as they approach, and whipping off the sides as they pass while the starfield streaks outward
tags: [milestone, tui, perspective, procgen, rendering]
deps: []
---

# Goal

## The scene

The terminal is the window of a spaceship flying forward through deep space. The frame is a near-black field stippled with stars, and **planets come at the ship out of the distance** — each emerging small near the vanishing point at the center, **growing as it approaches**, then **slewing off to the side** as it passes the glass. Each planet is a shaded sphere in its own color: a lit side, a terminator, and a dark side, drawn from a single point of light so the ball reads as round rather than flat.

Depth is real distance, not a style layer. The camera sits at the origin looking forward; the ship's motion shrinks every object's distance, and a single divide-by-distance projection does the rest — an off-axis planet's screen position races outward and its size swells as it nears, so it accelerates off the edge in the last moment. The stars ride the same projection and **streak outward** from the vanishing point, longer and brighter as they near — the warp field. A planet brightens as it closes, so the nearest pass is the brightest.

The scene runs forever. As an object reaches the camera it respawns far away at a fresh position, size, and color, so the flyby never repeats and never ends.

## How it renders

The terminal cell is twice as tall as it is wide, so planets drawn one-pixel-per-cell would be eggs. The renderer instead treats each cell as **two stacked pixels** and prints the upper-half block `▀`: the glyph's foreground color is the top pixel, its background color is the bottom. This doubles the vertical resolution and squares the pixels, so the spheres are round and every pixel carries its own truecolor.

## What it demonstrates

The milestone's first screensaver is reached when the scene runs on its own in a real terminal: planets emerging small at the vanishing point, growing as they approach and whipping off the sides as they pass, the starfield streaking outward, each planet a rounded multicolored sphere, the flyby renewing itself endlessly until the viewer quits.
