use bevy::prelude::*;

/// An elk's tint. Hues sweep the warm/purple arc — purple → magenta → red →
/// orange-brown — deliberately skipping the green/blue half of the wheel so a
/// pack never blends into the grass, water, or dirt behind it. Adjacent slots
/// alternate lightness so neighbouring hues still separate, and a grazing elk
/// dims just enough to read as feeding without losing its pack hue.
pub(crate) fn elk_color(slot: usize, grazing: bool) -> Color {
    const ARC: f32 = 120.0; // 285° (purple) through 405°≡45° (orange-brown)
    let hue = (285.0 + slot as f32 / (super::PACK_COUNT - 1) as f32 * ARC) % 360.0;
    let mut light = 0.5 + 0.1 * (slot % 2) as f32;
    if grazing {
        light *= 0.78; // a gentle dim, not a blackout
    }
    Color::hsl(hue, 0.7, light)
}
