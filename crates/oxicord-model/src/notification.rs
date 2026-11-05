use chrono::{DateTime, Utc};
use serde::Deserialize;

use crate::Snowflake;

/// Which messages in a place should notify us.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotifyLevel {
    All,
    Mentions,
    Nothing,
}

impl NotifyLevel {
    /// Discord's number for a level: 0 all, 1 mentions, 2 nothing. Anything
    /// else (3 means "inherit") is no level at all.
    pub fn from_wire(value: Option<u8>) -> Option<Self> {
        match value? {
            0 => Some(Self::All),
            1 => Some(Self::Mentions),
            2 => Some(Self::Nothing),
            _ => None,
        }
    }
}

/// A timed mute. No end time means until we unmute.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct MuteConfig {
    #[serde(default)]
    pub end_time: Option<DateTime<Utc>>,
}

/// A channel's own settings, which win over its server's.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct ChannelOverride {
    pub channel_id: Snowflake,
    #[serde(default)]
    pub muted: bool,
    #[serde(default)]
    pub mute_config: Option<MuteConfig>,
    #[serde(default)]
    pub message_notifications: Option<u8>,
}

/// One server's notification settings. `guild_id` is `None` for direct
/// messages.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct GuildNotificationSettings {
    #[serde(default)]
    pub guild_id: Option<Snowflake>,
    #[serde(default)]
    pub muted: bool,
    #[serde(default)]
    pub mute_config: Option<MuteConfig>,
    #[serde(default)]
    pub message_notifications: Option<u8>,
    #[serde(default)]
    pub suppress_everyone: bool,
    #[serde(default)]
    pub suppress_roles: bool,
    #[serde(default)]
    pub hide_muted_channels: bool,
    #[serde(default)]
    pub channel_overrides: Vec<ChannelOverride>,
}

/// Whether a mute is in force at `now`: switched on and either open ended
/// or not yet expired.
pub fn mute_active(muted: bool, config: Option<&MuteConfig>, now: DateTime<Utc>) -> bool {
    muted && config.and_then(|c| c.end_time).is_none_or(|end| end > now)
}

/// READY's copy of every server's settings.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct NotificationSettingsList {
    #[serde(default)]
    pub entries: Vec<GuildNotificationSettings>,
    /// True when the list is only the entries that changed.
    #[serde(default)]
    pub partial: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(s: &str) -> DateTime<Utc> {
        s.parse().unwrap()
    }

    #[test]
    fn a_mute_expires_at_its_end_time() {
        let config = MuteConfig { end_time: Some(at("2024-05-10T12:00:00Z")) };
        assert!(mute_active(true, Some(&config), at("2024-05-10T11:59:59Z")));
        assert!(!mute_active(true, Some(&config), at("2024-05-10T12:00:01Z")));
        assert!(mute_active(true, None, at("2030-01-01T00:00:00Z")));
        assert!(!mute_active(false, None, at("2024-05-10T11:00:00Z")));
    }

    #[test]
    fn levels_map_from_the_wire_numbers() {
        assert_eq!(NotifyLevel::from_wire(Some(0)), Some(NotifyLevel::All));
        assert_eq!(NotifyLevel::from_wire(Some(1)), Some(NotifyLevel::Mentions));
        assert_eq!(NotifyLevel::from_wire(Some(2)), Some(NotifyLevel::Nothing));
        assert_eq!(NotifyLevel::from_wire(Some(3)), None);
        assert_eq!(NotifyLevel::from_wire(None), None);
    }
}
