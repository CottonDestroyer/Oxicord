use serde::Deserialize;

use crate::Snowflake;

/// Per-channel read marker. `id` is the channel id.
#[derive(Clone, Debug, Deserialize)]
pub struct ReadState {
    pub id: Snowflake,
    #[serde(default)]
    pub last_message_id: Option<Snowflake>,
    #[serde(default)]
    pub mention_count: u32,
    /// When we last opened the pins of this channel.
    #[serde(default)]
    pub last_pin_timestamp: Option<String>,
}
