//! Preferences live in a plain JSON file in the user's config folder. Nothing
//! secret goes here, the login has its own home in `credentials`.

use std::path::PathBuf;

use oxicord_ui::Settings;

fn path() -> Option<PathBuf> {
    Some(dirs::config_dir()?.join("oxicord").join("settings.json"))
}

/// A missing or unreadable file just means the defaults.
pub fn load() -> Settings {
    let Some(path) = path() else {
        return Settings::default();
    };
    match std::fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_else(|err| {
            tracing::warn!(%err, "settings file is malformed, using defaults");
            Settings::default()
        }),
        Err(_) => Settings::default(),
    }
}

pub fn save(settings: &Settings) {
    let Some(path) = path() else { return };
    let result = path
        .parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        .and_then(|()| std::fs::write(&path, serde_json::to_vec_pretty(settings).unwrap_or_default()));
    if let Err(err) = result {
        tracing::warn!(%err, "could not save settings");
    }
}
