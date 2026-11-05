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
}
