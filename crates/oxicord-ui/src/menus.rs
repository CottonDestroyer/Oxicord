//! What the server and channel menus can do, and the requests each choice
//! becomes. Kept free of views so the request bodies can be checked.

use chrono::{DateTime, Duration, Utc};
use oxicord_model::{NotifyLevel, Snowflake};
use serde_json::{Value, json};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MuteFor {
    Minutes(i64),
    /// Until the user turns it back on.
    Forever,
}

impl MuteFor {
    pub const CHOICES: [(MuteFor, &'static str); 6] = [
        (MuteFor::Minutes(15), "For 15 Minutes"),
        (MuteFor::Minutes(60), "For 1 Hour"),
        (MuteFor::Minutes(180), "For 3 Hours"),
        (MuteFor::Minutes(480), "For 8 Hours"),
        (MuteFor::Minutes(1440), "For 24 Hours"),
        (MuteFor::Forever, "Until I turn it back on"),
    ];

    /// The choice a server's saved mute window matches, as the web client
    /// shows it in the duration select. No window means until turned back on.
    pub fn from_window(seconds: Option<i64>) -> MuteFor {
        match seconds {
            Some(s) if s > 0 && Self::CHOICES.iter().any(|(how, _)| *how == MuteFor::Minutes(s / 60)) => MuteFor::Minutes(s / 60),
            _ => MuteFor::Forever,
        }
    }

    pub fn label(self) -> &'static str {
        Self::CHOICES.iter().find(|(how, _)| *how == self).map_or("", |(_, label)| label)
    }

    /// The `mute_config` Discord expects: how long, and when it ends.
    fn config(self, now: DateTime<Utc>) -> Value {
        match self {
            MuteFor::Minutes(m) => json!({
                "selected_time_window": m * 60,
                "end_time": (now + Duration::minutes(m)).to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            }),
            MuteFor::Forever => json!({ "selected_time_window": -1, "end_time": null }),
        }
    }
}

/// What a menu entry does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    MarkRead,
    Mute(MuteFor),
    Unmute,
    /// `None` means "use the default", which for a channel is to inherit.
    SetLevel(Option<NotifyLevel>),
    SuppressEveryone(bool),
    SuppressRoles(bool),
    /// Switches the Highlights digest off (true) or on.
    SuppressHighlights(bool),
    MuteEvents(bool),
    MobilePush(bool),
    HideMutedChannels(bool),
    /// Takes a channel's own settings away so it follows its server again.
    ClearOverride,
    CopyLink,
    CopyId,
    CopyGuildId,
    Leave,
}

/// Which settings entry a request changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    Guild(Snowflake),
    Channel { guild: Snowflake, channel: Snowflake },
}

fn level_number(level: Option<NotifyLevel>) -> u8 {
    match level {
        Some(NotifyLevel::All) => 0,
        Some(NotifyLevel::Mentions) => 1,
        Some(NotifyLevel::Nothing) => 2,
        // Discord's number for "inherit".
        None => 3,
    }
}

/// The PATCH body for a settings action, or `None` when the action isn't a
/// settings change.
pub fn settings_body(place: Place, action: Action, now: DateTime<Utc>) -> Option<Value> {
    let fields = match action {
        Action::Mute(how) => json!({ "muted": true, "mute_config": how.config(now) }),
        Action::Unmute => json!({ "muted": false, "mute_config": null }),
        Action::SetLevel(level) => json!({ "message_notifications": level_number(level) }),
        Action::SuppressEveryone(on) => json!({ "suppress_everyone": on }),
        Action::SuppressRoles(on) => json!({ "suppress_roles": on }),
        // The client's enum: 1 disabled, 2 enabled.
        Action::SuppressHighlights(off) => json!({ "notify_highlights": if off { 1 } else { 2 } }),
        Action::MuteEvents(on) => json!({ "mute_scheduled_events": on }),
        Action::MobilePush(on) => json!({ "mobile_push": on }),
        Action::HideMutedChannels(on) => json!({ "hide_muted_channels": on }),
        Action::ClearOverride => json!({ "message_notifications": level_number(None), "muted": false }),
        Action::MarkRead | Action::CopyLink | Action::CopyId | Action::CopyGuildId | Action::Leave => return None,
    };
    Some(match place {
        Place::Guild(_) => fields,
        // A channel's settings sit under its id inside the server's entry.
        Place::Channel { channel, .. } => json!({ "channel_overrides": { channel.to_string(): fields } }),
    })
}

pub fn guild_of(place: Place) -> Snowflake {
    match place {
        Place::Guild(g) | Place::Channel { guild: g, .. } => g,
    }
}

/// The link Discord uses for a channel.
pub fn channel_link(guild: Option<Snowflake>, channel: Snowflake) -> String {
    match guild {
        Some(g) => format!("https://discord.com/channels/{g}/{channel}"),
        None => format!("https://discord.com/channels/@me/{channel}"),
    }
}

/// The link Discord uses for one message.
pub fn message_link(guild: Option<Snowflake>, channel: Snowflake, message: Snowflake) -> String {
    format!("{}/{message}", channel_link(guild, channel))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> DateTime<Utc> {
        "2024-05-10T12:00:00Z".parse().unwrap()
    }

    #[test]
    fn a_timed_mute_carries_its_window_and_end() {
        let body = settings_body(Place::Guild(Snowflake(1)), Action::Mute(MuteFor::Minutes(60)), now()).unwrap();
        assert_eq!(body["muted"], true);
        assert_eq!(body["mute_config"]["selected_time_window"], 3600);
        assert_eq!(body["mute_config"]["end_time"], "2024-05-10T13:00:00.000Z");
    }

    #[test]
    fn a_saved_window_picks_its_choice() {
        assert_eq!(MuteFor::from_window(Some(3600)), MuteFor::Minutes(60));
        assert_eq!(MuteFor::from_window(Some(86400)).label(), "For 24 Hours");
        assert_eq!(MuteFor::from_window(Some(-1)), MuteFor::Forever);
        assert_eq!(MuteFor::from_window(None), MuteFor::Forever);
        assert_eq!(MuteFor::from_window(Some(1234)), MuteFor::Forever);
    }

    #[test]
    fn muting_for_good_has_no_end() {
        let body = settings_body(Place::Guild(Snowflake(1)), Action::Mute(MuteFor::Forever), now()).unwrap();
        assert_eq!(body["mute_config"]["selected_time_window"], -1);
        assert!(body["mute_config"]["end_time"].is_null());
    }

    #[test]
    fn unmuting_clears_the_config() {
        let body = settings_body(Place::Guild(Snowflake(1)), Action::Unmute, now()).unwrap();
        assert_eq!(body, json!({ "muted": false, "mute_config": null }));
    }

    #[test]
    fn a_channel_change_is_nested_under_its_id() {
        let place = Place::Channel { guild: Snowflake(1), channel: Snowflake(7) };
        let body = settings_body(place, Action::SetLevel(Some(NotifyLevel::Mentions)), now()).unwrap();
        assert_eq!(body, json!({ "channel_overrides": { "7": { "message_notifications": 1 } } }));
        let inherit = settings_body(place, Action::SetLevel(None), now()).unwrap();
        assert_eq!(inherit["channel_overrides"]["7"]["message_notifications"], 3);
    }

    #[test]
    fn the_modal_switches_name_their_wire_fields() {
        let place = Place::Guild(Snowflake(1));
        assert_eq!(settings_body(place, Action::SuppressRoles(true), now()).unwrap(), json!({ "suppress_roles": true }));
        assert_eq!(settings_body(place, Action::MuteEvents(true), now()).unwrap(), json!({ "mute_scheduled_events": true }));
        assert_eq!(settings_body(place, Action::MobilePush(false), now()).unwrap(), json!({ "mobile_push": false }));
        assert_eq!(settings_body(place, Action::SuppressHighlights(true), now()).unwrap(), json!({ "notify_highlights": 1 }));
        assert_eq!(settings_body(place, Action::SuppressHighlights(false), now()).unwrap(), json!({ "notify_highlights": 2 }));
    }

    #[test]
    fn clearing_an_override_inherits_and_unmutes() {
        let place = Place::Channel { guild: Snowflake(1), channel: Snowflake(7) };
        let body = settings_body(place, Action::ClearOverride, now()).unwrap();
        assert_eq!(body, json!({ "channel_overrides": { "7": { "message_notifications": 3, "muted": false } } }));
    }

    #[test]
    fn non_settings_actions_have_no_body() {
        for action in [Action::MarkRead, Action::CopyLink, Action::Leave] {
            assert!(settings_body(Place::Guild(Snowflake(1)), action, now()).is_none());
        }
    }

    #[test]
    fn a_message_link_extends_the_channel_link() {
        assert_eq!(
            message_link(Some(Snowflake(1)), Snowflake(2), Snowflake(3)),
            "https://discord.com/channels/1/2/3"
        );
    }

    #[test]
    fn links_use_at_me_for_conversations() {
        assert_eq!(channel_link(Some(Snowflake(1)), Snowflake(2)), "https://discord.com/channels/1/2");
        assert_eq!(channel_link(None, Snowflake(2)), "https://discord.com/channels/@me/2");
    }
}
