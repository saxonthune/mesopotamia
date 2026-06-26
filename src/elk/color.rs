use bevy::prelude::*;

/// Purple→red arc, skipping orange/yellow (dirt hue). Alternating lightness separates adjacent slots.
pub(crate) fn elk_color(slot: usize, grazing: bool) -> Color {
    const ARC: f32 = 95.0; // 280° (purple) through 375°≡15° (red), clear of dirt-orange
    let hue = (280.0 + slot as f32 / (super::PACK_COUNT - 1) as f32 * ARC) % 360.0;
    let mut light = 0.52 + 0.1 * (slot % 2) as f32;
    if grazing {
        light *= 0.78; // a gentle dim, not a blackout
    }
    Color::hsl(hue, 0.85, light)
}
