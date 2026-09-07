//! Driftscape — a suite of terminal screensavers (doc03.03).
//!
//! Each screensaver is a drift field past a viewport: things have a position
//! and a velocity, advance by `dt`, and respawn when they leave an edge — the
//! same flow-past-a-frame motion the Bevy sim renders, here sampled onto a grid
//! of colored ASCII glyphs chosen by brightness.
//!
//! The scene modules are std-only and hold no terminal I/O. The crossterm shell
//! lives in the native `demo3` binary; on wasm, a small adapter exposes ANSI
//! frames to the browser's xterm.js frontend.

pub mod canvas;
pub mod color;
pub mod scene;
pub mod starliner;

#[cfg(target_arch = "wasm32")]
mod web;
