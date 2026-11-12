use serde::Deserialize;

use crate::{Channel, Emoji, Member, Role, Snowflake};

/// A guild in the shape the rest of the client wants.
///
/// The wire format has two layouts. Most payloads put `name`, `icon` and
/// friends at the top level, but some gateway payloads nest them under
/// `properties`. `RawGuild` reads either one.
#[derive(Clone, Debug, Deserialize)]
#[serde(from = "RawGuild")]
pub struct Guild {
    pub id: Snowflake,
    pub name: String,
    pub icon: Option<String>,
    /// The picture behind the name at the top of the channel list.
    pub banner: Option<String>,
    pub owner_id: Option<Snowflake>,
    pub member_count: Option<u64>,
    /// Boost level, 0 to 3.
    pub premium_tier: u8,
    /// How many boosts the server has.
    pub premium_subscription_count: u64,
    /// Whether the server shows its Boost Goal bar in the channel list.
    pub premium_progress_bar_enabled: bool,
    /// The short code that makes an invite link for the server, which lets
    /// members without the invite permission still hand out a link.
    pub vanity_url_code: Option<String>,
    pub unavailable: bool,
    pub roles: Vec<Role>,
    pub channels: Vec<Channel>,
    pub threads: Vec<Channel>,
    pub emojis: Vec<Emoji>,
    /// Stickers this server offers.
    pub stickers: Vec<crate::StickerItem>,
    /// 0 notify for every message, 1 only for mentions.
    pub default_message_notifications: u8,
    /// Only the members the server chose to include. For a user account
    /// that is usually just ourselves.
    pub members: Vec<Member>,
    /// Who sits in each voice channel when the payload arrives.
    pub voice_states: Vec<crate::VoiceState>,
    /// Events the server has planned or is running.
    pub scheduled_events: Vec<crate::ScheduledEvent>,
}

#[derive(Deserialize)]
struct RawGuild {
    id: Snowflake,
    #[serde(default)]
    properties: Option<GuildProperties>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    icon: Option<String>,
    #[serde(default)]
    banner: Option<String>,
    #[serde(default)]
    owner_id: Option<Snowflake>,
    #[serde(default)]
    member_count: Option<u64>,
    #[serde(default)]
    premium_tier: Option<u8>,
    #[serde(default)]
    premium_subscription_count: Option<u64>,
    #[serde(default)]
    premium_progress_bar_enabled: Option<bool>,
    #[serde(default)]
    vanity_url_code: Option<String>,
    #[serde(default)]
    unavailable: bool,
    #[serde(default)]
    roles: Vec<Role>,
    #[serde(default)]
    channels: Vec<Channel>,
    #[serde(default)]
    threads: Vec<Channel>,
    #[serde(default)]
    emojis: Vec<Emoji>,
    #[serde(default, deserialize_with = "crate::serde_util::lenient_vec")]
    stickers: Vec<crate::StickerItem>,
    #[serde(default)]
    default_message_notifications: Option<u8>,
    #[serde(default)]
    members: Vec<Member>,
    #[serde(default, deserialize_with = "crate::serde_util::lenient_vec")]
    voice_states: Vec<crate::VoiceState>,
    #[serde(default, deserialize_with = "crate::serde_util::lenient_vec")]
    guild_scheduled_events: Vec<crate::ScheduledEvent>,
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct GuildProperties {
    name: Option<String>,
    icon: Option<String>,
    banner: Option<String>,
    owner_id: Option<Snowflake>,
    member_count: Option<u64>,
    premium_tier: Option<u8>,
    premium_subscription_count: Option<u64>,
    premium_progress_bar_enabled: Option<bool>,
    vanity_url_code: Option<String>,
    default_message_notifications: Option<u8>,
}

impl From<RawGuild> for Guild {
    fn from(raw: RawGuild) -> Self {
        let props = raw.properties.unwrap_or_default();
        let mut channels = raw.channels;
        let mut threads = raw.threads;

        // Channels inside a guild payload leave out their own guild id.
        for channel in channels.iter_mut().chain(threads.iter_mut()) {
            channel.guild_id.get_or_insert(raw.id);
        }

        Guild {
            id: raw.id,
            name: raw.name.or(props.name).unwrap_or_default(),
            icon: raw.icon.or(props.icon),
            banner: raw.banner.or(props.banner),
            owner_id: raw.owner_id.or(props.owner_id),
            member_count: raw.member_count.or(props.member_count),
            premium_tier: raw.premium_tier.or(props.premium_tier).unwrap_or(0),
            premium_subscription_count: raw
                .premium_subscription_count
                .or(props.premium_subscription_count)
                .unwrap_or(0),
            premium_progress_bar_enabled: raw
                .premium_progress_bar_enabled
                .or(props.premium_progress_bar_enabled)
                .unwrap_or(false),
            vanity_url_code: raw.vanity_url_code.or(props.vanity_url_code),
            unavailable: raw.unavailable,
            roles: raw.roles,
            channels,
            threads,
            emojis: raw.emojis,
            stickers: raw.stickers,
            default_message_notifications: raw
                .default_message_notifications
                .or(props.default_message_notifications)
                .unwrap_or(0),
            members: raw.members,
            voice_states: raw.voice_states,
            scheduled_events: raw.guild_scheduled_events,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_flat_layout() {
        let g: Guild = serde_json::from_str(
            r#"{"id":"1","name":"flat","icon":"abc","owner_id":"9",
                "channels":[{"id":"2","type":0,"name":"general"}]}"#,
        )
        .unwrap();
        assert_eq!(g.name, "flat");
        assert_eq!(g.icon.as_deref(), Some("abc"));
        assert_eq!(g.channels[0].guild_id, Some(Snowflake(1)));
    }

    #[test]
    fn reads_nested_properties_layout() {
        let g: Guild = serde_json::from_str(
            r#"{"id":"1","properties":{"name":"nested","icon":null,"owner_id":"9"},
                "channels":[]}"#,
        )
        .unwrap();
        assert_eq!(g.name, "nested");
        assert_eq!(g.owner_id, Some(Snowflake(9)));
    }

    #[test]
    fn a_bad_voice_state_does_not_sink_the_guild() {
        let g: Guild = serde_json::from_str(
            r#"{"id":"1","voice_states":[{"user_id":"2","channel_id":"3"},{"nonsense":true},{"user_id":"4","member":"odd"}]}"#,
        )
        .unwrap();
        assert_eq!(g.voice_states.len(), 1);
    }

    #[test]
    fn stickers_are_read_and_a_bad_one_does_not_sink_the_guild() {
        let g: Guild = serde_json::from_str(
            r#"{"id":"1","stickers":[{"id":"2","name":"wave","format_type":1,"available":true,"tags":"wave"},{"name":"no id"}]}"#,
        )
        .unwrap();
        assert_eq!(g.stickers.len(), 1);
        assert_eq!(g.stickers[0].name, "wave");
    }

    #[test]
    fn scheduled_events_are_read_and_a_bad_one_does_not_sink_the_guild() {
        let g: Guild = serde_json::from_str(
            r#"{"id":"1","guild_scheduled_events":[
                {"id":"2","guild_id":"1","name":"a","scheduled_start_time":"2026-11-10T19:00:00+00:00"},
                {"name":"no id"}]}"#,
        )
        .unwrap();
        assert_eq!(g.scheduled_events.len(), 1);
        assert_eq!(g.scheduled_events[0].name, "a");
    }

    #[test]
    fn banner_and_boosts_are_read_from_either_layout() {
        let flat: Guild = serde_json::from_str(
            r#"{"id":"1","name":"a","banner":"b1","premium_tier":2,"premium_subscription_count":9}"#,
        )
        .unwrap();
        assert_eq!(flat.banner.as_deref(), Some("b1"));
        assert_eq!((flat.premium_tier, flat.premium_subscription_count), (2, 9));

        let nested: Guild = serde_json::from_str(
            r#"{"id":"1","properties":{"name":"a","banner":"b2","premium_tier":3,"premium_progress_bar_enabled":true},"premium_subscription_count":62}"#,
        )
        .unwrap();
        assert!(nested.premium_progress_bar_enabled && !flat.premium_progress_bar_enabled);
        assert_eq!(nested.banner.as_deref(), Some("b2"));
        assert_eq!((nested.premium_tier, nested.premium_subscription_count), (3, 62));
    }

    #[test]
    fn unavailable_guild_has_no_name() {
        let g: Guild = serde_json::from_str(r#"{"id":"1","unavailable":true}"#).unwrap();
        assert!(g.unavailable);
        assert!(g.name.is_empty());
    }
}
