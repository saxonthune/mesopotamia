---
title: Goal
summary: The first Driftscape screensaver — the terminal is a spaceship flying forward through space, planets emerging small at the vanishing point, growing as they approach, and whipping off the sides as they pass while the starfield streaks outward
tags: [milestone, tui, perspective, procgen, rendering]
deps: []
---

# Goal

## The scene

The terminal is the window of a spaceship flying forward through deep space. The frame is a near-black field stippled with stars, and **planets come at the ship out of the distance** — each emerging small near the vanishing point at the center, **growing as it approaches**, then **slewing off to the side** as it passes the glass. Each planet is a flat-lit disc in its own color — a bright outline ring with no dark side — carrying a single tight highlight where it faces the light, so it reads as a clean, glowing body rather than a fully modelled sphere.

Depth is real distance, not a style layer. The camera sits at the origin looking forward; the ship's motion shrinks every object's distance, and a single divide-by-distance projection does the rest — an off-axis planet's screen position races outward and its size swells as it nears, so it accelerates off the edge in the last moment. The stars ride the same projection and **streak outward** from the vanishing point, longer and brighter as they near — the warp field. A planet brightens as it closes, so the nearest pass is the brightest.

The scene runs forever. As an object reaches the camera it respawns far away at a fresh position, size, and color, so the flyby never repeats and never ends.

## How it renders

The terminal cell is twice as tall as it is wide, so planets drawn one-pixel-per-cell would be eggs. The renderer instead draws into an oversampled buffer where each cell is **two stacked square pixels**, keeping the spheres round. It then collapses each cell into a single **ASCII glyph**: the two pixels' combined brightness chooses a character from a dark-to-light ramp (` .,:;=+*#%@`) and the brighter pixel lends its truecolor. A planet resolves into colored ASCII art rather than solid color blocks: a flat-lit body with no dark side, ringed by a bright outline and carrying a tight specular spot where it faces the light. A gentle stipple hashed from the surface normal mottles the body into funky patches that ride the ball as it turns.

## What it demonstrates

The milestone's first screensaver is reached when the scene runs on its own in a real terminal: planets emerging small at the vanishing point, growing as they approach and whipping off the sides as they pass, the starfield streaking outward, each planet a clean multicolored disc outlined and highlighted, the flyby renewing itself endlessly until the viewer quits.
