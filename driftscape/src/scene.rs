//! The `Scene` trait — the seam the screensaver suite swaps on.
//!
//! A scene owns its own drift field. The binary advances it with `step(dt)` and
//! paints it with `draw(canvas)`; adding a new screensaver means adding a type
//! that implements this trait, nothing in the terminal shell changes.

use crate::canvas::Canvas;

pub trait Scene {
    /// Advance the field by `dt` seconds: move things, respawn what left the frame.
    fn step(&mut self, dt: f32);

    /// Paint the current state onto the canvas (which the caller has sized).
    fn draw(&self, canvas: &mut Canvas);

    /// Stable identifier, for selecting a scene by name.
    fn name(&self) -> &'static str;
}
