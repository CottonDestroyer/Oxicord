use serde::Deserialize;
use serde_json::Value;

use crate::{
    Activity, Channel, Emoji, Guild, GuildNotificationSettings, MemberList, Message,
    NotificationSettingsList, PartialMessage, ReadState, Relationship, RelationshipRemove, Role,
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
    #[serde(default)]
    pub relationships: Vec<Relationship>,
    #[serde(default, deserialize_with = "lenient_settings")]
    pub user_guild_settings: Option<NotificationSettingsList>,
}

/// READY has carried the notification settings both as `{ entries, .. }` and
/// as a bare list. They only refine what we show, so anything we can't read
/// is dropped instead of failing the whole login.
fn lenient_settings<'de, D>(deserializer: D) -> Result<Option<NotificationSettingsList>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<Value>::deserialize(deserializer)?;
    Ok(value.and_then(|v| match v {
        Value::Array(entries) => serde_json::from_value(serde_json::json!({ "entries": entries })).ok(),
        other => serde_json::from_value(other).ok(),
    }))
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
    #[serde(default)]
    pub merged_presences: Option<MergedPresences>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SupplementalGuild {
    pub id: Snowflake,
}

/// Who is online, delivered alongside READY. `friends` covers the people we
/// can see in DMs, `guilds` is indexed in parallel with the supplemental
/// guild list.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct MergedPresences {
    #[serde(default)]
    pub friends: Vec<PresenceEntry>,
    #[serde(default)]
    pub guilds: Vec<Vec<PresenceEntry>>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PresenceEntry {
    pub user_id: Snowflake,
    #[serde(default)]
    pub status: Status,
}

/// One slot of a guild's member list: a group header or a member.
#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum ListItem {
    Group { group: ListGroup },
    Member { member: ListMember },
}

#[derive(Clone, Debug, Deserialize)]
pub struct ListGroup {
    /// A role id, or "online" or "offline".
    pub id: String,
    #[serde(default)]
    pub count: u64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ListMember {
    pub user: User,
    #[serde(default)]
    pub nick: Option<String>,
    #[serde(default)]
    pub roles: Vec<Snowflake>,
    #[serde(default)]
    pub presence: Option<ListPresence>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ListPresence {
    #[serde(default)]
    pub status: Status,
    #[serde(default)]
    pub activities: Vec<Activity>,
}

/// An edit to the slots of a member list, in the order the server sent them.
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "op", rename_all = "UPPERCASE")]
pub enum ListOp {
    Sync { range: [usize; 2], items: Vec<ListItem> },
    Insert { index: usize, item: ListItem },
    Update { index: usize, item: ListItem },
    Delete { index: usize },
    Invalidate { range: [usize; 2] },
}

#[derive(Clone, Debug, Deserialize)]
pub struct MemberListUpdate {
    pub guild_id: Snowflake,
    /// Which list this is. Channels with the same visibility share one.
    pub id: String,
    #[serde(default)]
    pub member_count: Option<u64>,
    #[serde(default)]
    pub groups: Vec<ListGroup>,
    #[serde(default)]
    pub ops: Vec<ListOp>,
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

/// MESSAGE_REACTION_REMOVE_ALL and _EMOJI. The emoji is only present on the
/// second kind.
#[derive(Clone, Debug, Deserialize)]
pub struct ReactionsCleared {
    pub channel_id: Snowflake,
    pub message_id: Snowflake,
    #[serde(default)]
    pub emoji: Option<Emoji>,
}

/// Large servers send channel activity in bulk instead of one event per
/// message, so unread state depends on this.
#[derive(Clone, Debug, Deserialize)]
pub struct PassiveUpdate {
    pub guild_id: Snowflake,
    #[serde(default)]
    pub updated_channels: Vec<ChannelActivity>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ChannelActivity {
    pub id: Snowflake,
    #[serde(default)]
    pub last_message_id: Option<Snowflake>,
}

/// A person joining or leaving a group DM.
#[derive(Clone, Debug, Deserialize)]
pub struct RecipientChange {
    pub channel_id: Snowflake,
    pub user: User,
}

/// THREAD_LIST_SYNC: every active thread under some channels, sent when we
/// gain access to them.
#[derive(Clone, Debug, Deserialize)]
pub struct ThreadListSync {
    pub guild_id: Snowflake,
    /// The parents the list covers. Absent means the whole server.
    #[serde(default)]
    pub channel_ids: Option<Vec<Snowflake>>,
    #[serde(default)]
    pub threads: Vec<Channel>,
    #[serde(default)]
    pub members: Vec<ThreadMemberRef>,
}

/// Someone's membership of a thread. `id` is the thread.
#[derive(Clone, Debug, Deserialize)]
pub struct ThreadMemberRef {
    #[serde(default)]
    pub id: Option<Snowflake>,
    #[serde(default)]
    pub user_id: Option<Snowflake>,
    #[serde(default)]
    pub flags: u64,
}

/// THREAD_MEMBER_UPDATE, about ourselves, and THREAD_MEMBERS_UPDATE, about
/// anyone. Both are folded into one shape.
#[derive(Clone, Debug, Deserialize)]
pub struct ThreadMembersUpdate {
    pub id: Snowflake,
    #[serde(default)]
    pub member_count: Option<u32>,
    #[serde(default)]
    pub added_members: Vec<ThreadMemberRef>,
    #[serde(default)]
    pub removed_member_ids: Vec<Snowflake>,
    /// Set on THREAD_MEMBER_UPDATE, which describes our own membership.
    #[serde(default)]
    pub user_id: Option<Snowflake>,
    #[serde(default)]
    pub flags: u64,
}

/// MESSAGE_POLL_VOTE_ADD and MESSAGE_POLL_VOTE_REMOVE.
#[derive(Clone, Debug, Deserialize)]
pub struct PollVote {
    pub user_id: Snowflake,
    pub channel_id: Snowflake,
    pub message_id: Snowflake,
    pub answer_id: u32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct GuildEmojisUpdate {
    pub guild_id: Snowflake,
    #[serde(default)]
    pub emojis: Vec<Emoji>,
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
    VoiceStateUpdate(Box<crate::VoiceState>),
    RelationshipAdd(Box<Relationship>),
    RelationshipRemove(RelationshipRemove),
    ReactionsCleared(ReactionsCleared),
    PollVoteAdd(PollVote),
    PollVoteRemove(PollVote),
    PassiveUpdate(PassiveUpdate),
    CurrentUserUpdate(Box<User>),
    RecipientAdd(RecipientChange),
    RecipientRemove(RecipientChange),
    GuildEmojisUpdate(GuildEmojisUpdate),
    ThreadListSync(ThreadListSync),
    ThreadMembersUpdate(ThreadMembersUpdate),
    ThreadMemberUpdate(ThreadMembersUpdate),
    GuildSettingsUpdate(Box<GuildNotificationSettings>),
    TypingStart(TypingStart),
    GuildMemberListUpdate(Box<MemberListUpdate>),
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
            "VOICE_STATE_UPDATE" => Self::VoiceStateUpdate(Box::new(de(data)?)),
            "RELATIONSHIP_ADD" => Self::RelationshipAdd(Box::new(de(data)?)),
            "RELATIONSHIP_REMOVE" => Self::RelationshipRemove(de(data)?),
            "USER_GUILD_SETTINGS_UPDATE" => Self::GuildSettingsUpdate(Box::new(de(data)?)),
            "MESSAGE_REACTION_REMOVE_ALL" | "MESSAGE_REACTION_REMOVE_EMOJI" => {
                Self::ReactionsCleared(de(data)?)
            }
            "MESSAGE_POLL_VOTE_ADD" => Self::PollVoteAdd(de(data)?),
            "MESSAGE_POLL_VOTE_REMOVE" => Self::PollVoteRemove(de(data)?),
            "PASSIVE_UPDATE_V2" => Self::PassiveUpdate(de(data)?),
            "CURRENT_USER_UPDATE" => Self::CurrentUserUpdate(Box::new(de(data)?)),
            "CHANNEL_RECIPIENT_ADD" => Self::RecipientAdd(de(data)?),
            "CHANNEL_RECIPIENT_REMOVE" => Self::RecipientRemove(de(data)?),
            "THREAD_CREATE" => Self::ChannelCreate(Box::new(de(data)?)),
            "THREAD_UPDATE" => Self::ChannelUpdate(Box::new(de(data)?)),
            "THREAD_DELETE" => Self::ChannelDelete(Box::new(de(data)?)),
            "THREAD_LIST_SYNC" => Self::ThreadListSync(de(data)?),
            "THREAD_MEMBERS_UPDATE" => Self::ThreadMembersUpdate(de(data)?),
            "THREAD_MEMBER_UPDATE" => Self::ThreadMemberUpdate(de(data)?),
            "GUILD_EMOJIS_UPDATE" => Self::GuildEmojisUpdate(de(data)?),
            "TYPING_START" => Self::TypingStart(de(data)?),
            "GUILD_MEMBER_LIST_UPDATE" => Self::GuildMemberListUpdate(Box::new(de(data)?)),
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
    fn parses_member_list_updates_with_every_op_kind() {
        let event = DispatchEvent::parse(
            "GUILD_MEMBER_LIST_UPDATE",
            json!({
                "guild_id": "5", "id": "everyone", "member_count": 40,
                "groups": [{ "id": "online", "count": 2 }, { "id": "offline", "count": 38 }],
                "ops": [
                    { "op": "SYNC", "range": [0, 2], "items": [
                        { "group": { "id": "online", "count": 2 } },
                        { "member": { "user": { "id": "1", "username": "a" },
                                      "roles": ["9"], "presence": { "status": "online" } } },
                        { "member": { "user": { "id": "2", "username": "b" } } }
                    ] },
                    { "op": "INSERT", "index": 1, "item": { "member": { "user": { "id": "3", "username": "c" } } } },
                    { "op": "UPDATE", "index": 0, "item": { "group": { "id": "online", "count": 3 } } },
                    { "op": "DELETE", "index": 2 },
                    { "op": "INVALIDATE", "range": [0, 99] }
                ]
            }),
        )
        .unwrap();

        let DispatchEvent::GuildMemberListUpdate(u) = event else {
            panic!("wrong variant");
        };
        assert_eq!(u.ops.len(), 5);
        assert_eq!(u.groups[1].count, 38);
        let ListOp::Sync { items, .. } = &u.ops[0] else { panic!("expected SYNC first") };
        assert!(matches!(items[0], ListItem::Group { .. }));
        assert!(matches!(&items[1], ListItem::Member { member } if member.roles == vec![Snowflake(9)]));
    }

    #[test]
    fn ready_supplemental_carries_friend_presences() {
        let event = DispatchEvent::parse(
            "READY_SUPPLEMENTAL",
            json!({ "guilds": [], "merged_presences": {
                "friends": [{ "user_id": "7", "status": "dnd" }], "guilds": [[]] } }),
        )
        .unwrap();
        let DispatchEvent::ReadySupplemental(s) = event else { panic!("wrong variant") };
        let friends = s.merged_presences.unwrap().friends;
        assert_eq!(friends[0].status, Status::Dnd);
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

#[cfg(test)]
mod ready_tolerance {
    use serde_json::json;

    use super::*;

    fn ready(settings: serde_json::Value) -> Result<Ready, serde_json::Error> {
        serde_json::from_value(json!({
            "user": { "id": "1", "username": "me" }, "session_id": "s",
            "user_guild_settings": settings
        }))
    }

    #[test]
    fn notification_settings_never_break_ready() {
        let object = ready(json!({ "entries": [{ "guild_id": "5", "muted": true }], "partial": false })).unwrap();
        assert_eq!(object.user_guild_settings.unwrap().entries.len(), 1);

        let list = ready(json!([{ "guild_id": "5", "muted": true }])).unwrap();
        assert_eq!(list.user_guild_settings.unwrap().entries.len(), 1);

        // Nonsense is ignored rather than rejected.
        assert!(ready(json!("surprise")).unwrap().user_guild_settings.is_none());
        assert!(ready(json!({ "entries": { "unexpected": true } })).unwrap().user_guild_settings.is_none());
    }
}
