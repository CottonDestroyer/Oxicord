use serde::Deserialize;
use serde_repr::Deserialize_repr;

use crate::Emoji;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Online,
    Idle,
    Dnd,
    #[default]
    #[serde(other)]
    Offline,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize_repr)]
#[repr(u8)]
pub enum ActivityType {
    #[default]
    Playing = 0,
    Streaming = 1,
    Listening = 2,
    Watching = 3,
    Custom = 4,
    Competing = 5,
    #[serde(other)]
    Unknown = 255,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Activity {
    pub name: String,
    #[serde(rename = "type", default)]
    pub kind: ActivityType,
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub details: Option<String>,
    #[serde(default)]
    pub emoji: Option<Emoji>,
    #[serde(default)]
    pub timestamps: Option<ActivityTimestamps>,
    // Only used for pictures, so an odd value must not cost the whole
    // presence, which arrives inside the login payload.
    #[serde(default, deserialize_with = "crate::serde_util::lenient_default")]
    pub application_id: Option<crate::Snowflake>,
    #[serde(default, deserialize_with = "crate::serde_util::lenient_default")]
    pub assets: Option<ActivityAssets>,
}

/// The pictures an activity carries.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct ActivityAssets {
    #[serde(default)]
    pub large_image: Option<String>,
    #[serde(default)]
    pub large_text: Option<String>,
}

/// When an activity began and, for something with a set length, ends.
/// Both are unix milliseconds.
#[derive(Clone, Copy, Debug, Default, Deserialize)]
pub struct ActivityTimestamps {
    #[serde(default)]
    pub start: Option<u64>,
    #[serde(default)]
    pub end: Option<u64>,
}

impl Activity {
    /// The words Discord puts before the name, as in "Playing Minecraft".
    /// Custom statuses have none, they are the person's own words.
    pub fn verb(&self) -> Option<&'static str> {
        match self.kind {
            ActivityType::Playing => Some("Playing"),
            ActivityType::Streaming => Some("Streaming"),
            ActivityType::Listening => Some("Listening to"),
            ActivityType::Watching => Some("Watching"),
            ActivityType::Competing => Some("Competing in"),
            ActivityType::Custom | ActivityType::Unknown => None,
        }
    }

    /// The address of the big picture, whichever service it lives on.
    /// Spotify and proxied images are named by a prefix, an application's
    /// own art by its id.
    pub fn image_url(&self) -> Option<String> {
        let image = self.assets.as_ref()?.large_image.as_deref().filter(|i| !i.is_empty())?;
        if let Some(rest) = image.strip_prefix("spotify:") {
            Some(format!("https://i.scdn.co/image/{rest}"))
        } else if let Some(rest) = image.strip_prefix("mp:external/") {
            Some(format!("https://media.discordapp.net/external/{rest}"))
        } else {
            let app = self.application_id?;
            Some(format!("https://cdn.discordapp.com/app-assets/{app}/{image}.png"))
        }
    }

    pub fn is_custom_status(&self) -> bool {
        self.kind == ActivityType::Custom
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activities_read_with_and_without_timestamps() {
        let game: Activity = serde_json::from_value(serde_json::json!({
            "name": "Minecraft", "type": 0, "timestamps": { "start": 1_700_000_000_000u64 }
        }))
        .unwrap();
        assert_eq!(game.verb(), Some("Playing"));
        assert_eq!(game.timestamps.and_then(|t| t.start), Some(1_700_000_000_000));

        let custom: Activity =
            serde_json::from_value(serde_json::json!({ "name": "Custom Status", "type": 4, "state": "busy" })).unwrap();
        assert!(custom.is_custom_status() && custom.verb().is_none());
        assert!(custom.timestamps.is_none());
    }

    #[test]
    fn the_picture_address_follows_the_prefix() {
        let make = |image: &str| -> Activity {
            serde_json::from_value(serde_json::json!({
                "name": "x", "type": 2, "application_id": "42", "assets": { "large_image": image }
            }))
            .unwrap()
        };
        assert_eq!(make("spotify:abc").image_url().as_deref(), Some("https://i.scdn.co/image/abc"));
        assert_eq!(
            make("mp:external/k/https/h/p.png").image_url().as_deref(),
            Some("https://media.discordapp.net/external/k/https/h/p.png")
        );
        assert_eq!(make("9f").image_url().as_deref(), Some("https://cdn.discordapp.com/app-assets/42/9f.png"));
        // Odd shapes are dropped without losing the activity.
        let odd: Activity = serde_json::from_value(serde_json::json!({
            "name": "x", "application_id": {"nope": 1}, "assets": "text"
        }))
        .unwrap();
        assert_eq!((odd.application_id, odd.image_url()), (None, None));
        let bare: Activity = serde_json::from_value(serde_json::json!({ "name": "x" })).unwrap();
        assert_eq!(bare.image_url(), None);
    }
}
