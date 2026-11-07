//! Serves Discord's own icons and its font from inside the executable, and
//! everything else from the bundled catalog.
//!
//! The files live in the `discord-assets` folder at the repository root and
//! are compiled in. They are Discord's, kept here because this is a personal
//! project. If the folder were ever removed the build would still work and
//! draw the bundled icons and the system font. Debug builds read the folder
//! from disk, so a changed icon shows without rebuilding.

use std::borrow::Cow;

use gpui_kit::{AssetSource, SharedString};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "../../discord-assets"]
#[allow_missing = true]
struct Embedded;

/// The gg sans files, as bytes ready to register with the text system.
pub fn fonts() -> Vec<Cow<'static, [u8]>> {
    Embedded::iter()
        .filter(|name| name.starts_with("fonts/") && name.ends_with(".ttf"))
        .filter_map(|name| Embedded::get(&name))
        .map(|file| file.data)
        .collect()
}

pub struct Assets<A> {
    inner: A,
}

impl<A> Assets<A> {
    pub fn new(inner: A) -> Self {
        Self { inner }
    }
}

impl<A: AssetSource> AssetSource for Assets<A> {
    fn load(&self, path: &str) -> gpui_kit::Result<Option<Cow<'static, [u8]>>> {
        if let Some(file) = Embedded::get(path) {
            return Ok(Some(file.data));
        }
        // These only exist in Discord's folder. Without it, draw a similar
        // bundled icon instead of failing on every frame.
        let path = match path {
            "icons/friends.svg" => "icons/users.svg",
            "icons/clyde.svg" => "icons/bot.svg",
            "icons/soundboard.svg" | "icons/voice-noise.svg" => "icons/audio-lines.svg",
            "icons/voice-hangup.svg" => "icons/phone-off.svg",
            "icons/voice-signal.svg" => "icons/signal.svg",
            other => other,
        };
        self.inner.load(path)
    }

    fn list(&self, path: &str) -> gpui_kit::Result<Vec<SharedString>> {
        self.inner.list(path)
    }
}
