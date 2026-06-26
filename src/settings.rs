//! Launch settings from `usersettings.json` (gitignored; falls back to defaults when absent).
//! On wasm, `load` is a no-op — defaults always apply.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct WindowSettings {
    pub maximized: bool,
    pub width: u32,
    pub height: u32,
}

impl Default for WindowSettings {
    fn default() -> Self {
        Self { maximized: true, width: 1280, height: 720 }
    }
}

#[derive(Resource, Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct UserSettings {
    pub window: WindowSettings,
}

impl UserSettings {
    pub const FILE: &'static str = "usersettings.json";

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

// Bevy has no construction-time maximize field, so this must be a startup system.
#[cfg(not(target_arch = "wasm32"))]
pub fn maximize_window(
    settings: Res<UserSettings>,
    window: Single<&mut Window, With<PrimaryWindow>>,
) {
    if settings.window.maximized {
        window.into_inner().set_maximized(true);
    }
}
