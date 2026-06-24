//! Driftscape — a suite of terminal screensavers (doc03.03).
//!
//! Each screensaver is a drift field past a viewport: things have a position
//! and a velocity, advance by `dt`, and respawn when they leave an edge — the
//! same flow-past-a-frame motion the Bevy sim renders, here sampled onto a grid
//! of colored ASCII glyphs chosen by brightness.
//!
//! These modules are std-only and hold no terminal I/O. The crossterm shell —
//! raw mode, alternate screen, sizing, input, the frame loop — lives in the
//! native-only `demo3` binary, so everything here stays pure and testable.

pub mod canvas;
pub mod color;
pub mod scene;
pub mod starliner;
