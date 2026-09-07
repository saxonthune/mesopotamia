use bevy::prelude::*;

use crate::elk::{cell_water_penalty, grass_gradient, ElkParams};
use crate::events::EventLog;
use crate::grid::Grid;
use crate::render::{cell_world_pos, TILE_SIZE};
use crate::ui::{Overlay, UiState};

pub struct OverlayPlugin;

impl Plugin for OverlayPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                draw_grass_gradient_overlay
                    .run_if(|s: Res<UiState>| s.overlays.contains(&Overlay::GrassGradient)),
                draw_water_penalty_overlay
                    .run_if(|s: Res<UiState>| s.overlays.contains(&Overlay::WaterPenalty)),
                draw_death_sites_overlay
                    .run_if(|s: Res<UiState>| s.overlays.contains(&Overlay::DeathSites)),
            ),
        );
    }
}

fn draw_grass_gradient_overlay(
    mut gizmos: Gizmos,
    grid: Res<Grid>,
    params: Res<ElkParams>,
) {
    const STEP: usize = 4;
    const ARROW_LEN: f32 = TILE_SIZE * 2.0;

    for row in (0..grid.height()).step_by(STEP) {
        for col in (0..grid.width()).step_by(STEP) {
            let cell = row * grid.width() + col;
            let g = grass_gradient(cell, &grid, &params);
            let len = g.length();
            if len < 1e-6 {
                continue;
            }
            let dir = g / len;
            let origin = cell_world_pos(&grid, cell);
            let tip = origin + dir * ARROW_LEN;
            gizmos.line_2d(origin, tip, Color::srgba(0.2, 0.9, 0.2, 0.75));
            // Arrowhead: two short lines at ~45° back from the tip
            let head_len = ARROW_LEN * 0.3;
            let perp = Vec2::new(-dir.y, dir.x);
            let back = tip - dir * head_len;
            gizmos.line_2d(tip, back + perp * head_len * 0.5, Color::srgba(0.2, 0.9, 0.2, 0.75));
            gizmos.line_2d(tip, back - perp * head_len * 0.5, Color::srgba(0.2, 0.9, 0.2, 0.75));
        }
    }
}

fn draw_water_penalty_overlay(
    mut gizmos: Gizmos,
    grid: Res<Grid>,
    params: Res<ElkParams>,
) {
    let max_penalty = params.water_cost.max(1e-6);

    for index in 0..grid.len() {
        if grid.water(index) < 0.01 {
            continue;
        }
        let penalty = cell_water_penalty(index, &grid, &params);
        let alpha = (penalty / max_penalty).clamp(0.0, 1.0) * 0.8;
        let color = Color::srgba(0.9, 0.15, 0.15, alpha);
        let pos = cell_world_pos(&grid, index);
        let half = TILE_SIZE * 0.5;
        let tl = pos + Vec2::new(-half,  half);
        let tr = pos + Vec2::new( half,  half);
        let bl = pos + Vec2::new(-half, -half);
        let br = pos + Vec2::new( half, -half);
        gizmos.line_2d(tl, tr, color);
        gizmos.line_2d(tr, br, color);
        gizmos.line_2d(br, bl, color);
        gizmos.line_2d(bl, tl, color);
    }
}

fn draw_death_sites_overlay(
    mut gizmos: Gizmos,
    grid: Res<Grid>,
    event_log: Res<EventLog>,
) {
    let total = event_log.recent.len();
    if total == 0 {
        return;
    }
    let half = TILE_SIZE * 0.4;
    for (i, event) in event_log.recent.iter().enumerate() {
        let age_frac = i as f32 / total as f32; // 0 = oldest, 1 = newest
        let alpha = 0.2 + age_frac * 0.6;
        let color = Color::srgba(1.0, 0.1, 0.1, alpha);
        let pos = cell_world_pos(&grid, event.cell);
        gizmos.line_2d(pos + Vec2::new(-half, -half), pos + Vec2::new(half, half), color);
        gizmos.line_2d(pos + Vec2::new(-half, half), pos + Vec2::new(half, -half), color);
    }
}
