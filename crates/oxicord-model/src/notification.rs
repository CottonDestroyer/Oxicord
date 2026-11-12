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
    /// How long the mute was asked for, in seconds, or -1 for no end.
    #[serde(default)]
    pub selected_time_window: Option<i64>,
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

impl ChannelOverride {
    /// Whether the channel departs from its server's settings at all. A
    /// cleared override stays in the list with the "inherit" level and no mute.
    pub fn is_set(&self) -> bool {
        self.muted || NotifyLevel::from_wire(self.message_notifications).is_some()
    }
}

/// One server's notification settings. `guild_id` is `None` for direct
/// messages.
#[derive(Clone, Debug, Deserialize)]
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
    pub mute_scheduled_events: bool,
    /// Push to the phone. On unless the server's entry says otherwise.
    #[serde(default = "on")]
    pub mobile_push: bool,
    /// 0 unset, 1 off, 2 on: the Highlights digest for this server.
    #[serde(default)]
    pub notify_highlights: u8,
    #[serde(default, deserialize_with = "overrides")]
    pub channel_overrides: Vec<ChannelOverride>,
}

fn on() -> bool {
    true
}

impl Default for GuildNotificationSettings {
    fn default() -> Self {
        Self {
            guild_id: None,
            muted: false,
            mute_config: None,
            message_notifications: None,
            suppress_everyone: false,
            suppress_roles: false,
            hide_muted_channels: false,
            mute_scheduled_events: false,
            mobile_push: true,
            notify_highlights: 0,
            channel_overrides: Vec::new(),
        }
    }
}

impl GuildNotificationSettings {
    /// Whether Highlights are switched off for the server.
    pub fn highlights_off(&self) -> bool {
        self.notify_highlights == 1
    }
}

/// The server sends overrides as a list in some payloads and as an object
/// keyed by channel id in others, so accept both.
fn overrides<'de, D>(deserializer: D) -> Result<Vec<ChannelOverride>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;
    use serde_json::Value;

    let entry = |key: Option<String>, mut value: Value| -> Result<ChannelOverride, serde_json::Error> {
        if let (Some(key), Value::Object(map)) = (key, &mut value) {
            map.entry("channel_id").or_insert(Value::String(key));
        }
        serde_json::from_value(value)
    };
    match Option::<Value>::deserialize(deserializer)? {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(list)) => list.into_iter().map(|v| entry(None, v)).collect(),
        Some(Value::Object(map)) => map.into_iter().map(|(k, v)| entry(Some(k), v)).collect(),
        Some(_) => return Err(D::Error::custom("expected channel overrides")),
    }
    .map_err(D::Error::custom)
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
        let config = MuteConfig { end_time: Some(at("2024-05-10T12:00:00Z")), ..Default::default() };
        assert!(mute_active(true, Some(&config), at("2024-05-10T11:59:59Z")));
        assert!(!mute_active(true, Some(&config), at("2024-05-10T12:00:01Z")));
        assert!(mute_active(true, None, at("2030-01-01T00:00:00Z")));
        assert!(!mute_active(false, None, at("2024-05-10T11:00:00Z")));
    }

    #[test]
    fn overrides_may_be_a_list_or_a_map_keyed_by_channel() {
        let list: GuildNotificationSettings = serde_json::from_str(
            r#"{"guild_id":"1","channel_overrides":[{"channel_id":"5","muted":true}]}"#,
        )
        .unwrap();
        let map: GuildNotificationSettings = serde_json::from_str(
            r#"{"guild_id":"1","channel_overrides":{"5":{"channel_id":"5","muted":true},"6":{"muted":false}}}"#,
        )
        .unwrap();
        assert_eq!(list.channel_overrides.len(), 1);
        assert_eq!(map.channel_overrides.len(), 2);
        assert!(map.channel_overrides.iter().any(|o| o.channel_id == Snowflake(6)));
        let none: GuildNotificationSettings = serde_json::from_str(r#"{"channel_overrides":null}"#).unwrap();
        assert!(none.channel_overrides.is_empty());
    }

    #[test]
    fn the_extra_switches_read_from_the_entry_and_push_defaults_on() {
        let bare: GuildNotificationSettings = serde_json::from_str(r#"{"guild_id":"1"}"#).unwrap();
        assert!(bare.mobile_push && !bare.mute_scheduled_events && !bare.highlights_off());
        let set: GuildNotificationSettings = serde_json::from_str(
            r#"{"guild_id":"1","mobile_push":false,"mute_scheduled_events":true,"notify_highlights":1}"#,
        )
        .unwrap();
        assert!(!set.mobile_push && set.mute_scheduled_events && set.highlights_off());
        assert!(GuildNotificationSettings::default().mobile_push);
    }

    #[test]
    fn a_cleared_override_is_not_set() {
        let ov = |muted: bool, level: Option<u8>| ChannelOverride { muted, message_notifications: level, ..Default::default() };
        assert!(!ov(false, None).is_set());
        assert!(!ov(false, Some(3)).is_set());
        assert!(ov(true, Some(3)).is_set());
        assert!(ov(false, Some(1)).is_set());
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
