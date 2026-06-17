use bevy::prelude::*;

/// An elk's tint. Hues sweep the purple→red arc — purple → magenta → red —
/// deliberately skipping both the green/blue half of the wheel AND the
/// orange/yellow band (≈30–60°), which is the tan-dirt hue a pack would vanish
/// into. High saturation keeps the dots reading against grass, water, and dirt
/// alike. Adjacent slots alternate lightness so neighbouring hues still
/// separate, and a grazing elk dims just enough to read as feeding without
/// losing its pack hue.
pub(crate) fn elk_color(slot: usize, grazing: bool) -> Color {
    const ARC: f32 = 95.0; // 280° (purple) through 375°≡15° (red), clear of dirt-orange
    let hue = (280.0 + slot as f32 / (super::PACK_COUNT - 1) as f32 * ARC) % 360.0;
    let mut light = 0.52 + 0.1 * (slot % 2) as f32;
    if grazing {
        light *= 0.78; // a gentle dim, not a blackout
    }
    Color::hsl(hue, 0.85, light)
}
