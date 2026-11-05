use serde::Deserialize;
use serde_json::Value;

use crate::{
    Activity, Channel, Emoji, Guild, MemberList, Message, PartialMessage, ReadState, Role,
    Snowflake, Status, User,
};

pub mod op {
    pub const DISPATCH: u8 = 0;
    pub const HEARTBEAT: u8 = 1;
    pub const IDENTIFY: u8 = 2;
    pub const PRESENCE_UPDATE: u8 = 3;
    pub const VOICE_STATE_UPDATE: u8 = 4;
    pub const RESUME: u8 = 6;
    pub const RECONNECT: u8 = 7;
    pub const REQUEST_GUILD_MEMBERS: u8 = 8;
    pub const INVALID_SESSION: u8 = 9;
    pub const HELLO: u8 = 10;
    pub const HEARTBEAT_ACK: u8 = 11;
    pub const GUILD_SUBSCRIPTIONS_BULK: u8 = 37;
}

/// The envelope every gateway frame uses.
#[derive(Clone, Debug, Deserialize)]
pub struct GatewayPayload {
    pub op: u8,
    #[serde(default)]
    pub d: Value,
    /// Sequence number, only set on dispatches.
    #[serde(default)]
    pub s: Option<u64>,
    #[serde(default)]
    pub t: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Hello {
    pub heartbeat_interval: u64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Ready {
    pub user: User,
    pub session_id: String,
    #[serde(default)]
    pub resume_gateway_url: Option<String>,
    #[serde(default)]
    pub guilds: Vec<Guild>,
    #[serde(default)]
    pub private_channels: Vec<Channel>,
    #[serde(default)]
    pub read_state: Option<ReadStates>,
    /// Every user mentioned elsewhere in the payload, listed once. Private
    /// channels only carry `recipient_ids` and rely on this to resolve them.
    #[serde(default)]
    pub users: Vec<User>,
    /// Indexed in parallel with `guilds`. For a user session each entry is
    /// our own member in that guild.
    #[serde(default)]
    pub merged_members: Vec<MemberList>,
}

/// Follows READY with data that didn't fit in it. All we use is
/// `merged_members`, which is indexed in parallel with this event's own
/// `guilds` list (not READY's).
#[derive(Clone, Debug, Deserialize)]
pub struct ReadySupplemental {
    #[serde(default)]
    pub guilds: Vec<SupplementalGuild>,
    #[serde(default)]
    pub merged_members: Vec<MemberList>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SupplementalGuild {
    pub id: Snowflake,
}

/// User sessions get read states as `{ entries: [...] }`. Accept a bare
/// array too in case the wrapper goes away.
#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum ReadStates {
    List(Vec<ReadState>),
    Wrapped { entries: Vec<ReadState> },
}

impl ReadStates {
    pub fn entries(&self) -> &[ReadState] {
        match self {
            Self::List(entries) | Self::Wrapped { entries } => entries,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct MessageDelete {
    pub id: Snowflake,
    pub channel_id: Snowflake,
    #[serde(default)]
    pub guild_id: Option<Snowflake>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct MessageDeleteBulk {
    pub ids: Vec<Snowflake>,
    pub channel_id: Snowflake,
    #[serde(default)]
    pub guild_id: Option<Snowflake>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct GuildDelete {
    pub id: Snowflake,
    /// Set when the guild went offline, absent when we were removed from it.
    #[serde(default)]
    pub unavailable: Option<bool>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct TypingStart {
    pub channel_id: Snowflake,
    #[serde(default)]
    pub guild_id: Option<Snowflake>,
    pub user_id: Snowflake,
    /// Unix seconds.
    pub timestamp: u64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ReactionEvent {
    pub user_id: Snowflake,
    pub channel_id: Snowflake,
    pub message_id: Snowflake,
    #[serde(default)]
    pub guild_id: Option<Snowflake>,
    pub emoji: Emoji,
}

#[derive(Clone, Debug, Deserialize)]
pub struct MessageAck {
    pub channel_id: Snowflake,
    pub message_id: Snowflake,
    #[serde(default)]
    pub mention_count: Option<u32>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PresenceUser {
    pub id: Snowflake,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PresenceUpdate {
    pub user: PresenceUser,
    #[serde(default)]
    pub guild_id: Option<Snowflake>,
    #[serde(default)]
    pub status: Status,
    #[serde(default)]
    pub activities: Vec<Activity>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct GuildMemberUpdate {
    pub guild_id: Snowflake,
    pub user: PresenceUser,
    #[serde(default)]
    pub roles: Vec<Snowflake>,
    #[serde(default)]
    pub nick: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct GuildRoleEvent {
    pub guild_id: Snowflake,
    pub role: Role,
}

#[derive(Clone, Debug, Deserialize)]
pub struct GuildRoleDelete {
    pub guild_id: Snowflake,
    pub role_id: Snowflake,
}

/// Dispatch events the client reacts to. Anything else is kept as `Unknown`
/// so new server events never turn into hard errors.
#[derive(Clone, Debug)]
pub enum DispatchEvent {
    Ready(Box<Ready>),
    ReadySupplemental(Box<ReadySupplemental>),
    Resumed,
    MessageCreate(Box<Message>),
    MessageUpdate(Box<PartialMessage>),
    MessageDelete(MessageDelete),
    MessageDeleteBulk(MessageDeleteBulk),
    MessageAck(MessageAck),
    ReactionAdd(ReactionEvent),
    ReactionRemove(ReactionEvent),
    GuildCreate(Box<Guild>),
    GuildUpdate(Box<Guild>),
    GuildDelete(GuildDelete),
    GuildMemberUpdate(GuildMemberUpdate),
    GuildRoleCreate(GuildRoleEvent),
    GuildRoleUpdate(GuildRoleEvent),
    GuildRoleDelete(GuildRoleDelete),
    ChannelCreate(Box<Channel>),
    ChannelUpdate(Box<Channel>),
    ChannelDelete(Box<Channel>),
    PresenceUpdate(PresenceUpdate),
    TypingStart(TypingStart),
    Unknown { name: String, data: Value },
}

impl DispatchEvent {
    pub fn parse(name: &str, data: Value) -> Result<Self, serde_json::Error> {
        use serde_json::from_value as de;

        Ok(match name {
            "READY" => Self::Ready(Box::new(de(data)?)),
            "READY_SUPPLEMENTAL" => Self::ReadySupplemental(Box::new(de(data)?)),
            "RESUMED" => Self::Resumed,
            "MESSAGE_CREATE" => Self::MessageCreate(Box::new(de(data)?)),
            "MESSAGE_UPDATE" => Self::MessageUpdate(Box::new(de(data)?)),
            "MESSAGE_DELETE" => Self::MessageDelete(de(data)?),
            "MESSAGE_DELETE_BULK" => Self::MessageDeleteBulk(de(data)?),
            "MESSAGE_ACK" => Self::MessageAck(de(data)?),
            "MESSAGE_REACTION_ADD" => Self::ReactionAdd(de(data)?),
            "MESSAGE_REACTION_REMOVE" => Self::ReactionRemove(de(data)?),
            "GUILD_CREATE" => Self::GuildCreate(Box::new(de(data)?)),
            "GUILD_UPDATE" => Self::GuildUpdate(Box::new(de(data)?)),
            "GUILD_DELETE" => Self::GuildDelete(de(data)?),
            "GUILD_MEMBER_UPDATE" => Self::GuildMemberUpdate(de(data)?),
            "GUILD_ROLE_CREATE" => Self::GuildRoleCreate(de(data)?),
            "GUILD_ROLE_UPDATE" => Self::GuildRoleUpdate(de(data)?),
            "GUILD_ROLE_DELETE" => Self::GuildRoleDelete(de(data)?),
            "CHANNEL_CREATE" => Self::ChannelCreate(Box::new(de(data)?)),
            "CHANNEL_UPDATE" => Self::ChannelUpdate(Box::new(de(data)?)),
            "CHANNEL_DELETE" => Self::ChannelDelete(Box::new(de(data)?)),
            "PRESENCE_UPDATE" => Self::PresenceUpdate(de(data)?),
            "TYPING_START" => Self::TypingStart(de(data)?),
            _ => Self::Unknown {
                name: name.to_owned(),
                data,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn parses_message_create() {
        let event = DispatchEvent::parse(
            "MESSAGE_CREATE",
            json!({
                "id": "10", "channel_id": "20", "guild_id": "30",
                "author": { "id": "40", "username": "ada", "global_name": "Ada" },
                "content": "hello",
                "timestamp": "2024-05-01T12:00:00.000000+00:00",
                "type": 0,
                "nonce": "123"
            }),
        )
        .unwrap();

        let DispatchEvent::MessageCreate(m) = event else {
            panic!("wrong variant");
        };
        assert_eq!(m.content, "hello");
        assert_eq!(m.author.display_name(), "Ada");
        assert_eq!(m.guild_id, Some(Snowflake(30)));
    }

    #[test]
    fn partial_update_keeps_missing_fields_none() {
        let event = DispatchEvent::parse(
            "MESSAGE_UPDATE",
            json!({ "id": "10", "channel_id": "20", "embeds": [] }),
        )
        .unwrap();

        let DispatchEvent::MessageUpdate(m) = event else {
            panic!("wrong variant");
        };
        assert!(m.content.is_none());
        assert!(m.embeds.is_some());
    }

    #[test]
    fn parses_ready_with_wrapped_read_state() {
        let event = DispatchEvent::parse(
            "READY",
            json!({
                "v": 9,
                "user": { "id": "1", "username": "me" },
                "session_id": "abc",
                "resume_gateway_url": "wss://gateway-us-east1-b.discord.gg",
                "guilds": [{ "id": "5", "properties": { "name": "home" }, "channels": [] }],
                "private_channels": [{ "id": "6", "type": 1, "recipient_ids": ["7"] }],
                "read_state": { "entries": [{ "id": "6", "last_message_id": "99", "mention_count": 2 }] }
            }),
        )
        .unwrap();

        let DispatchEvent::Ready(r) = event else {
            panic!("wrong variant");
        };
        assert_eq!(r.guilds[0].name, "home");
        assert_eq!(r.private_channels[0].recipient_ids, vec![Snowflake(7)]);
        assert_eq!(r.read_state.unwrap().entries()[0].mention_count, 2);
    }

    #[test]
    fn ready_reads_merged_members_as_array_or_object_per_guild() {
        let event = DispatchEvent::parse(
            "READY",
            json!({
                "user": { "id": "1", "username": "me" },
                "session_id": "abc",
                "guilds": [{ "id": "5" }, { "id": "6" }],
                "merged_members": [
                    [{ "user_id": "1", "roles": ["10", "11"] }],
                    { "user_id": "1", "roles": ["12"] }
                ]
            }),
        )
        .unwrap();

        let DispatchEvent::Ready(r) = event else {
            panic!("wrong variant");
        };
        let lists: Vec<_> = r.merged_members.into_iter().map(|m| m.into_vec()).collect();
        assert_eq!(lists[0][0].roles.len(), 2);
        assert_eq!(lists[1][0].roles, vec![Snowflake(12)]);
    }

    #[test]
    fn parses_ready_supplemental_members_by_guild_index() {
        let event = DispatchEvent::parse(
            "READY_SUPPLEMENTAL",
            json!({
                "guilds": [{ "id": "5", "voice_states": [] }, { "id": "6" }],
                "merged_members": [[{ "user_id": "1", "roles": ["10"] }], []],
                "merged_presences": { "friends": [] },
                "disclose": ["pomelo"]
            }),
        )
        .unwrap();

        let DispatchEvent::ReadySupplemental(s) = event else {
            panic!("wrong variant");
        };
        assert_eq!(s.guilds[1].id, Snowflake(6));
        let first = s.merged_members.into_iter().next().unwrap().into_vec();
        assert_eq!(first[0].roles, vec![Snowflake(10)]);
    }

    #[test]
    fn parses_role_and_member_updates() {
        let member = DispatchEvent::parse(
            "GUILD_MEMBER_UPDATE",
            json!({ "guild_id": "5", "user": { "id": "1" }, "roles": ["9"] }),
        )
        .unwrap();
        assert!(matches!(member, DispatchEvent::GuildMemberUpdate(m) if m.roles == vec![Snowflake(9)]));

        let role = DispatchEvent::parse(
            "GUILD_ROLE_UPDATE",
            json!({ "guild_id": "5", "role": { "id": "9", "name": "mods", "permissions": "8" } }),
        )
        .unwrap();
        assert!(matches!(role, DispatchEvent::GuildRoleUpdate(r) if r.role.name == "mods"));
    }

    #[test]
    fn unknown_event_is_kept_not_rejected() {
        let event = DispatchEvent::parse("SOMETHING_NEW", json!({ "x": 1 })).unwrap();
        assert!(matches!(event, DispatchEvent::Unknown { .. }));
    }

}
