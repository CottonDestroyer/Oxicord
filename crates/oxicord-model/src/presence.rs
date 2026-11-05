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
}
