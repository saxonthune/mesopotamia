//! User-tunable launch settings, read from `usersettings.json` at the repo root.
//!
//! The file is gitignored; `usersettings.example.json` is the checked-in
//! template. Anything missing falls back to [`Default`], so a partial file (or
//! no file at all) still launches. On the web there is no filesystem and the
//! window is sized by its host canvas, so [`UserSettings::load`] is a no-op
//! there and returns the defaults.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use serde::Deserialize;

/// Initial window geometry.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct WindowSettings {
    /// Maximize to fill the monitor's work area (keeping the OS title bar)
    /// once the window opens. When true, `width`/`height` are the size the
    /// window restores to when un-maximized.
    pub maximized: bool,
    pub width: u32,
    pub height: u32,
}

impl Default for WindowSettings {
    fn default() -> Self {
        Self { maximized: true, width: 1280, height: 720 }
    }
}

/// Top-level settings resource, loaded once at startup.
#[derive(Resource, Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct UserSettings {
    pub window: WindowSettings,
}

impl UserSettings {
    /// Name looked up in the current working directory (the repo root when run
    /// via `cargo run`).
    pub const FILE: &'static str = "usersettings.json";

    /// Read and parse `usersettings.json`, falling back to defaults when the
    /// file is absent or malformed.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn load() -> Self {
        match std::fs::read_to_string(Self::FILE) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_else(|e| {
                eprintln!("{}: parse error ({e}); using defaults", Self::FILE);
                Self::default()
            }),
            Err(_) => Self::default(), // no file → defaults, no warning
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn load() -> Self {
        Self::default()
    }
}

/// Startup system: maximize the primary window if the settings ask for it.
/// Maximizing is a runtime request in Bevy (there is no construction-time
/// field), so it happens here rather than in the `WindowPlugin` config.
#[cfg(not(target_arch = "wasm32"))]
pub fn maximize_window(
    settings: Res<UserSettings>,
    window: Single<&mut Window, With<PrimaryWindow>>,
) {
    if settings.window.maximized {
        window.into_inner().set_maximized(true);
    }
}
