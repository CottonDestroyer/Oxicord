use oxicord_model::{Message, Snowflake, Status, gateway::DispatchEvent};
use oxicord_store::{Changes, Store};
use serde_json::{Value, json};

const ME: u64 = 1;
const ADA: u64 = 2;
const GUILD: u64 = 100;

fn id(n: u64) -> Snowflake {
    Snowflake(n)
}

fn event(name: &str, data: Value) -> DispatchEvent {
    DispatchEvent::parse(name, data).unwrap()
}

fn perm(bits: u64) -> String {
    bits.to_string()
}

/// One guild with a category, a hidden channel and a voice channel, plus a
/// DM with Ada. Channel 11 has unread messages.
fn ready_json() -> Value {
    let view_send = perm(1 << 10 | 1 << 11 | 1 << 16);
    json!({
        "user": { "id": "1", "username": "me" },
        "session_id": "s",
        "users": [{ "id": "2", "username": "ada", "global_name": "Ada" }],
        "guilds": [{
            "id": "100",
            "properties": { "name": "home", "owner_id": "9" },
            "roles": [
                { "id": "100", "name": "@everyone", "permissions": view_send },
                { "id": "200", "name": "mods", "permissions": "0" }
            ],
            "channels": [
                { "id": "10", "type": 4, "name": "Text", "position": 0 },
                { "id": "11", "type": 0, "name": "general", "parent_id": "10", "position": 1,
                  "last_message_id": "600" },
                { "id": "12", "type": 0, "name": "announce", "parent_id": "10", "position": 0 },
                { "id": "13", "type": 2, "name": "voice", "parent_id": "10", "position": 0 },
                { "id": "14", "type": 0, "name": "secret", "parent_id": "10", "position": 2,
                  "permission_overwrites": [
                      { "id": "100", "type": 0, "allow": "0", "deny": perm(1 << 10) }
                  ] },
                { "id": "15", "type": 0, "name": "loose", "position": 0 }
            ]
        }],
        "merged_members": [[{ "user_id": "1", "roles": ["200"] }]],
        "private_channels": [
            { "id": "50", "type": 1, "recipient_ids": ["2"], "last_message_id": "900" }
        ],
        "read_state": { "entries": [{ "id": "11", "last_message_id": "500", "mention_count": 0 }] }
    })
}

fn ready_store() -> Store {
    let mut store = Store::new();
    store.apply(event("READY", ready_json()));
    store
}

fn message_json(channel: u64, mid: u64, author: u64) -> Value {
    json!({
        "id": mid.to_string(), "channel_id": channel.to_string(),
        "guild_id": if channel == 50 { Value::Null } else { json!("100") },
        "author": { "id": author.to_string(), "username": format!("user{author}") },
        "content": format!("msg {mid}"),
        "timestamp": "2024-05-01T12:00:00+00:00"
    })
}

fn create(channel: u64, mid: u64, author: u64) -> DispatchEvent {
    event("MESSAGE_CREATE", message_json(channel, mid, author))
}

fn history(channel: u64, ids: &[u64]) -> Vec<Message> {
    ids.iter()
        .map(|m| serde_json::from_value(message_json(channel, *m, ADA)).unwrap())
        .collect()
}

fn timeline_ids(store: &Store, channel: u64) -> Vec<u64> {
    store
        .timeline(id(channel))
        .unwrap()
        .messages()
        .iter()
        .map(|m| m.id.0)
        .collect()
}

#[test]
fn ready_loads_guilds_channels_and_identity() {
    let store = ready_store();
    assert_eq!(store.me().unwrap().username, "me");

    let guild = store.guilds().next().unwrap();
    assert_eq!(guild.name, "home");
    assert_eq!(store.channel(id(11)).unwrap().guild_id, Some(id(GUILD)));
    assert_eq!(store.my_roles(id(GUILD)), &[id(200)]);
}

#[test]
fn ready_resolves_dm_recipients_from_the_user_table() {
    let store = ready_store();
    let dms = store.dm_channels();
    assert_eq!(dms.len(), 1);
    assert_eq!(store.dm_name(dms[0]), "Ada");
}

#[test]
fn sidebar_groups_by_category_and_hides_what_we_cannot_see() {
    let store = ready_store();
    let groups = store.channel_groups(id(GUILD));

    assert_eq!(groups.len(), 2);
    // Uncategorized channels lead.
    assert!(groups[0].category.is_none());
    assert_eq!(groups[0].channels[0].id, id(15));

    let category = &groups[1];
    assert_eq!(category.category.unwrap().id, id(10));
    let order: Vec<_> = category.channels.iter().map(|c| c.id.0).collect();
    // Text by position, then voice. The secret channel (14) is gone.
    assert_eq!(order, [12, 11, 13]);
}

#[test]
fn unread_compares_last_message_with_read_marker() {
    let store = ready_store();
    assert!(store.unread(id(11)).unread);
    assert!(!store.unread(id(12)).unread);
    assert!(store.guild_unread(id(GUILD)).unread);
}

#[test]
fn message_in_an_unopened_channel_only_moves_unread_state() {
    let mut store = ready_store();
    let changes = store.apply(create(12, 700, ADA));

    assert!(store.timeline(id(12)).is_none());
    assert!(store.unread(id(12)).unread);
    assert!(changes.channel_lists.contains(&id(GUILD)));
    assert!(changes.messages.is_empty());
}

#[test]
fn messages_in_an_open_channel_stay_ordered_and_unique() {
    let mut store = ready_store();
    store.open_channel(id(11));

    store.apply(create(11, 702, ADA));
    store.apply(create(11, 701, ADA));
    store.apply(create(11, 702, ADA));

    assert_eq!(timeline_ids(&store, 11), [701, 702]);
}

#[test]
fn only_real_mentions_raise_the_badge() {
    let mut store = ready_store();

    store.apply(create(11, 701, ADA));
    assert_eq!(store.unread(id(11)).mentions, 0);

    let mut direct = message_json(11, 702, ADA);
    direct["mentions"] = json!([{ "id": "1", "username": "me" }]);
    store.apply(event("MESSAGE_CREATE", direct));

    let mut everyone = message_json(11, 703, ADA);
    everyone["mention_everyone"] = json!(true);
    store.apply(event("MESSAGE_CREATE", everyone));

    let mut role = message_json(11, 704, ADA);
    role["mention_roles"] = json!(["200"]);
    store.apply(event("MESSAGE_CREATE", role));

    let mut other_role = message_json(11, 705, ADA);
    other_role["mention_roles"] = json!(["999"]);
    store.apply(event("MESSAGE_CREATE", other_role));

    assert_eq!(store.unread(id(11)).mentions, 3);
}

#[test]
fn every_dm_message_from_someone_else_counts_as_a_mention() {
    let mut store = ready_store();
    store.apply(create(50, 901, ADA));
    assert_eq!(store.unread(id(50)).mentions, 1);
}

#[test]
fn sending_a_message_reads_the_channel() {
    let mut store = ready_store();
    assert!(store.unread(id(11)).unread);

    store.apply(create(11, 700, ME));
    assert!(!store.unread(id(11)).unread);
}

#[test]
fn first_message_in_a_channel_without_read_state_shows_as_unread() {
    let mut store = ready_store();
    assert!(!store.unread(id(12)).unread);
    store.apply(create(12, 700, ADA));
    assert!(store.unread(id(12)).unread);
}

#[test]
fn ack_event_clears_unread_and_mentions() {
    let mut store = ready_store();
    store.apply(create(11, 700, ADA));
    assert!(store.unread(id(11)).unread);

    let changes = store.apply(event(
        "MESSAGE_ACK",
        json!({ "channel_id": "11", "message_id": "700" }),
    ));
    assert!(!store.unread(id(11)).unread);
    assert!(changes.channel_lists.contains(&id(GUILD)));
}

#[test]
fn partial_update_changes_only_the_fields_it_carries() {
    let mut store = ready_store();
    store.open_channel(id(11));
    store.apply(create(11, 701, ADA));

    store.apply(event(
        "MESSAGE_UPDATE",
        json!({ "id": "701", "channel_id": "11", "embeds": [{ "title": "preview" }] }),
    ));

    let msg = store.timeline(id(11)).unwrap().get(id(701)).unwrap();
    assert_eq!(msg.content, "msg 701");
    assert_eq!(msg.embeds[0].title.as_deref(), Some("preview"));

    store.apply(event(
        "MESSAGE_UPDATE",
        json!({ "id": "701", "channel_id": "11", "content": "edited",
                "edited_timestamp": "2024-05-01T12:05:00+00:00" }),
    ));
    let msg = store.timeline(id(11)).unwrap().get(id(701)).unwrap();
    assert_eq!(msg.content, "edited");
    assert!(msg.edited_timestamp.is_some());
    assert_eq!(msg.embeds.len(), 1);
}

#[test]
fn deletes_remove_single_and_bulk() {
    let mut store = ready_store();
    store.open_channel(id(11));
    for m in [701, 702, 703, 704] {
        store.apply(create(11, m, ADA));
    }

    store.apply(event("MESSAGE_DELETE", json!({ "id": "701", "channel_id": "11" })));
    store.apply(event(
        "MESSAGE_DELETE_BULK",
        json!({ "ids": ["702", "703"], "channel_id": "11" }),
    ));
    assert_eq!(timeline_ids(&store, 11), [704]);
}

#[test]
fn reactions_add_and_remove_with_our_own_flag() {
    let mut store = ready_store();
    store.open_channel(id(11));
    store.apply(create(11, 701, ADA));

    let react = |name: &str, user: &str| json!({
        "user_id": user, "channel_id": "11", "message_id": "701", "emoji": { "name": name }
    });

    store.apply(event("MESSAGE_REACTION_ADD", react("👍", "2")));
    store.apply(event("MESSAGE_REACTION_ADD", react("👍", "1")));
    store.apply(event("MESSAGE_REACTION_ADD", react("🎉", "2")));

    let msg = store.timeline(id(11)).unwrap().get(id(701)).unwrap();
    assert_eq!(msg.reactions.len(), 2);
    assert_eq!(msg.reactions[0].count, 2);
    assert!(msg.reactions[0].me);

    store.apply(event("MESSAGE_REACTION_REMOVE", react("👍", "1")));
    store.apply(event("MESSAGE_REACTION_REMOVE", react("🎉", "2")));

    let msg = store.timeline(id(11)).unwrap().get(id(701)).unwrap();
    assert_eq!(msg.reactions.len(), 1, "an emoji with no reactions left disappears");
    assert_eq!(msg.reactions[0].count, 1);
    assert!(!msg.reactions[0].me);
}

#[test]
fn history_pages_merge_in_order_and_report_whether_more_exists() {
    let mut store = ready_store();

    // Discord returns newest first.
    store.set_history(id(11), history(11, &[530, 520, 510]), 3);
    assert_eq!(timeline_ids(&store, 11), [510, 520, 530]);
    assert!(store.timeline(id(11)).unwrap().has_more_before());

    store.set_history(id(11), history(11, &[505, 500]), 3);
    assert_eq!(timeline_ids(&store, 11), [500, 505, 510, 520, 530]);
    assert!(
        !store.timeline(id(11)).unwrap().has_more_before(),
        "a short page means we hit the start of the channel"
    );
}

#[test]
fn removed_guild_takes_its_channels_with_it_but_an_outage_does_not() {
    let mut store = ready_store();

    store.apply(event("GUILD_DELETE", json!({ "id": "100", "unavailable": true })));
    assert!(store.guild(id(GUILD)).unwrap().unavailable);
    assert!(store.channel(id(11)).is_some());

    let changes = store.apply(event("GUILD_DELETE", json!({ "id": "100" })));
    assert!(store.guild(id(GUILD)).is_none());
    assert!(store.channel(id(11)).is_none());
    assert!(changes.guild_list);
}

#[test]
fn gaining_a_role_can_reveal_a_channel() {
    let mut store = ready_store();
    // Make the secret channel visible only to role 300.
    store.apply(event(
        "CHANNEL_UPDATE",
        json!({ "id": "14", "type": 0, "guild_id": "100", "parent_id": "10",
                "permission_overwrites": [
                    { "id": "100", "type": 0, "allow": "0", "deny": perm(1 << 10) },
                    { "id": "300", "type": 0, "allow": perm(1 << 10), "deny": "0" }
                ] }),
    ));
    let visible = |s: &Store| {
        s.channel_groups(id(GUILD))
            .iter()
            .flat_map(|g| g.channels.iter())
            .any(|c| c.id == id(14))
    };
    assert!(!visible(&store));

    let changes = store.apply(event(
        "GUILD_MEMBER_UPDATE",
        json!({ "guild_id": "100", "user": { "id": "1" }, "roles": ["200", "300"] }),
    ));
    assert!(visible(&store));
    assert!(changes.affects_channel_list(id(GUILD)));
}

#[test]
fn channel_update_without_last_message_keeps_the_old_one() {
    let mut store = ready_store();
    store.apply(event(
        "CHANNEL_UPDATE",
        json!({ "id": "11", "type": 0, "guild_id": "100", "name": "renamed" }),
    ));
    let channel = store.channel(id(11)).unwrap();
    assert_eq!(channel.name.as_deref(), Some("renamed"));
    assert_eq!(channel.last_message_id, Some(id(600)));
}

#[test]
fn typing_expires_and_stops_when_the_user_posts() {
    let mut store = ready_store();
    store.open_channel(id(11));

    let typing = |user: &str| event("TYPING_START", json!({
        "channel_id": "11", "user_id": user, "timestamp": 1000
    }));
    store.apply(typing("2"));
    store.apply(typing("1"));

    assert_eq!(store.typing_users(id(11), 1005).len(), 1, "we never show ourselves");
    assert!(store.typing_users(id(11), 1011).is_empty());

    store.apply(typing("2"));
    let changes = store.apply(create(11, 701, ADA));
    assert!(store.typing_users(id(11), 1005).is_empty());
    assert!(changes.typing.contains(&id(11)));
}

#[test]
fn presence_tracks_status_and_forgets_offline_users() {
    let mut store = ready_store();
    let update = |status: &str| event("PRESENCE_UPDATE", json!({
        "user": { "id": "2" }, "status": status
    }));

    let changes = store.apply(update("dnd"));
    assert_eq!(store.presence(id(ADA)), Status::Dnd);
    assert!(changes.presences);

    store.apply(update("offline"));
    assert_eq!(store.presence(id(ADA)), Status::Offline);
}

#[test]
fn ready_wipes_state_from_the_previous_session() {
    let mut store = ready_store();
    store.open_channel(id(11));
    store.apply(create(11, 701, ADA));

    let changes = store.apply(event("READY", ready_json()));
    assert!(changes.full);
    assert!(store.timeline(id(11)).is_none());
}

#[test]
fn changes_merge_for_batched_redraws() {
    let mut a = Changes::default();
    a.messages.insert(id(1));
    let mut b = Changes {
        dm_list: true,
        ..Changes::default()
    };
    b.messages.insert(id(2));

    a.merge(b);
    assert!(a.dm_list);
    assert!(a.affects_messages(id(1)) && a.affects_messages(id(2)));
    assert!(!a.is_empty());
}
