use serde::Deserialize;

use crate::{Member, Snowflake};

/// Who is in a voice channel and how they are set up. Joining one is not
/// supported, but the list of who is in it is shown in the channel list.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct VoiceState {
    pub user_id: Snowflake,
    /// `None` once the person left voice.
    #[serde(default)]
    pub channel_id: Option<Snowflake>,
    /// Missing inside a guild payload, where the guild is implied.
    #[serde(default)]
    pub guild_id: Option<Snowflake>,
    #[serde(default)]
    pub self_mute: bool,
    #[serde(default)]
    pub self_deaf: bool,
    /// Muted or deafened by a moderator.
    #[serde(default)]
    pub mute: bool,
    #[serde(default)]
    pub deaf: bool,
    #[serde(default)]
    pub self_video: bool,
    #[serde(default)]
    pub self_stream: bool,
    /// Update events carry the member, which is how we learn new faces.
    #[serde(default)]
    pub member: Option<Member>,
}

impl VoiceState {
    pub fn muted(&self) -> bool {
        self.self_mute || self.mute
    }

    pub fn deafened(&self) -> bool {
        self.self_deaf || self.deaf
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_state_from_a_guild_payload() {
        let v: VoiceState =
            serde_json::from_str(r#"{"user_id":"7","channel_id":"9","self_mute":true,"mute":false}"#).unwrap();
        assert_eq!(v.channel_id, Some(Snowflake(9)));
        assert!(v.muted() && !v.deafened());
    }

    #[test]
    fn a_state_without_a_channel_means_the_person_left() {
        let v: VoiceState = serde_json::from_str(r#"{"user_id":"7","channel_id":null,"guild_id":"1"}"#).unwrap();
        assert!(v.channel_id.is_none());
    }
}
