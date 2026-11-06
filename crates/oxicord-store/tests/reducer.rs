use oxicord_model::{Message, RelationshipKind, Snowflake, Status, gateway::DispatchEvent};
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
fn last_read_follows_acks() {
    let mut store = ready_store();
    assert_eq!(store.last_read(id(11)), Some(id(500)));
    assert_eq!(store.last_read(id(12)), None);

    store.mark_read(id(11), id(600));
    assert_eq!(store.last_read(id(11)), Some(id(600)));
    assert!(!store.unread(id(11)).unread);
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
fn batched_reactions_count_every_user_and_notice_us() {
    let mut store = ready_store();
    store.open_channel(id(11));
    store.apply(create(11, 701, ADA));

    let changes = store.apply(event(
        "MESSAGE_REACTION_ADD_MANY",
        json!({
            "channel_id": "11", "message_id": "701",
            "reactions": [
                { "emoji": { "name": "👍" }, "users": ["2", "3", "1"] },
                { "emoji": { "name": "🎉" }, "users": ["2"] },
                { "emoji": { "name": "🔥" }, "users": [] },
            ]
        }),
    ));
    assert!(changes.messages.contains(&id(11)));

    let msg = store.timeline(id(11)).unwrap().get(id(701)).unwrap();
    assert_eq!(msg.reactions.len(), 2, "an emoji nobody used is not shown");
    assert_eq!(msg.reactions[0].count, 3);
    assert!(msg.reactions[0].me);
    assert!(!msg.reactions[1].me);
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
fn supplemental_members_supply_our_roles_when_ready_had_none() {
    let mut ready = ready_json();
    ready["merged_members"] = json!([]);
    // Make the secret channel visible only to role 300.
    ready["guilds"][0]["channels"][4]["permission_overwrites"] = json!([
        { "id": "100", "type": 0, "allow": "0", "deny": perm(1 << 10) },
        { "id": "300", "type": 0, "allow": perm(1 << 10), "deny": "0" }
    ]);

    let mut store = Store::new();
    store.apply(event("READY", ready));
    let visible = |s: &Store| {
        s.channel_groups(id(GUILD))
            .iter()
            .flat_map(|g| g.channels.iter())
            .any(|c| c.id == id(14))
    };
    assert!(store.my_roles(id(GUILD)).is_empty());
    assert!(!visible(&store));

    let changes = store.apply(event(
        "READY_SUPPLEMENTAL",
        json!({
            "guilds": [{ "id": "100" }],
            "merged_members": [[{ "user_id": "1", "roles": ["300"] }]]
        }),
    ));
    assert_eq!(store.my_roles(id(GUILD)), &[id(300)]);
    assert!(visible(&store));
    assert!(changes.affects_channel_list(id(GUILD)));
}

#[test]
fn supplemental_members_for_someone_else_are_ignored() {
    let mut store = ready_store();
    store.apply(event(
        "READY_SUPPLEMENTAL",
        json!({
            "guilds": [{ "id": "100" }],
            "merged_members": [[{ "user_id": "2", "roles": ["999"] }]]
        }),
    ));
    assert_eq!(store.my_roles(id(GUILD)), &[id(200)]);
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
fn activities_follow_presence_and_go_with_it() {
    let mut store = ready_store();
    let update = |status: &str, activities: serde_json::Value| event("PRESENCE_UPDATE", json!({
        "user": { "id": "2" }, "status": status, "activities": activities
    }));

    store.apply(update("online", json!([{ "name": "Minecraft", "type": 0 }, { "broken": true }])));
    assert_eq!(store.activities(id(ADA)).len(), 1);
    assert_eq!(store.activities(id(ADA))[0].name, "Minecraft");

    store.apply(update("online", json!(null)));
    assert!(store.activities(id(ADA)).is_empty());

    store.apply(update("online", json!([{ "name": "Minecraft", "type": 0 }])));
    store.apply(update("offline", json!([{ "name": "Minecraft", "type": 0 }])));
    assert!(store.activities(id(ADA)).is_empty());
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

fn member(id: u64, name: &str) -> Value {
    json!({ "member": { "user": { "id": id.to_string(), "username": name },
                        "roles": ["200"], "presence": { "status": "online" } } })
}

fn group(id: &str, count: u64) -> Value {
    json!({ "group": { "id": id, "count": count } })
}

fn list_update(list_id: &str, ops: Value) -> DispatchEvent {
    event(
        "GUILD_MEMBER_LIST_UPDATE",
        json!({ "guild_id": "100", "id": list_id, "member_count": 50, "ops": ops }),
    )
}

/// The user ids and group names in slot order, "-" for an empty slot.
fn describe(store: &Store) -> Vec<String> {
    use oxicord_store::Entry;
    store
        .member_list(id(GUILD))
        .unwrap()
        .entries()
        .iter()
        .map(|slot| match slot {
            Some(Entry::Group { id, .. }) => format!("g:{id}"),
            Some(Entry::Member { user, .. }) => format!("m:{}", user.0),
            None => "-".to_owned(),
        })
        .collect()
}

#[test]
fn member_list_sync_fills_slots_and_teaches_the_store_about_users() {
    let mut store = ready_store();
    let changes = store.apply(list_update(
        "everyone",
        json!([{ "op": "SYNC", "range": [0, 2],
                 "items": [group("online", 2), member(10, "ann"), member(11, "bo")] }]),
    ));

    assert_eq!(describe(&store), ["g:online", "m:10", "m:11"]);
    assert_eq!(store.user(id(10)).unwrap().username, "ann");
    assert_eq!(store.presence(id(10)), Status::Online);
    assert_eq!(store.member_list(id(GUILD)).unwrap().member_count(), Some(50));
    assert!(changes.members.contains(&id(GUILD)));
    assert!(changes.presences);
}

#[test]
fn member_list_insert_update_delete_and_invalidate_edit_in_place() {
    let mut store = ready_store();
    store.apply(list_update(
        "everyone",
        json!([{ "op": "SYNC", "range": [0, 3],
                 "items": [group("online", 3), member(10, "a"), member(11, "b"), member(12, "c")] }]),
    ));

    store.apply(list_update("everyone", json!([{ "op": "INSERT", "index": 2, "item": member(20, "new") }])));
    assert_eq!(describe(&store), ["g:online", "m:10", "m:20", "m:11", "m:12"]);

    store.apply(list_update("everyone", json!([{ "op": "UPDATE", "index": 0, "item": group("online", 4) }])));
    store.apply(list_update("everyone", json!([{ "op": "DELETE", "index": 1 }])));
    assert_eq!(describe(&store), ["g:online", "m:20", "m:11", "m:12"]);

    store.apply(list_update("everyone", json!([{ "op": "INVALIDATE", "range": [1, 2] }])));
    assert_eq!(describe(&store), ["g:online", "-", "-", "m:12"]);
}

#[test]
fn a_second_sync_range_extends_the_list() {
    let mut store = ready_store();
    store.apply(list_update("everyone", json!([{ "op": "SYNC", "range": [0, 1],
        "items": [group("online", 3), member(10, "a")] }])));
    store.apply(list_update("everyone", json!([{ "op": "SYNC", "range": [4, 5],
        "items": [member(14, "e"), member(15, "f")] }])));

    assert_eq!(describe(&store), ["g:online", "m:10", "-", "-", "m:14", "m:15"]);
}

#[test]
fn a_different_list_id_starts_the_list_over() {
    let mut store = ready_store();
    store.apply(list_update("everyone", json!([{ "op": "SYNC", "range": [0, 1],
        "items": [group("online", 1), member(10, "a")] }])));
    store.apply(list_update("abc123", json!([{ "op": "SYNC", "range": [0, 0],
        "items": [group("offline", 5)] }])));

    assert_eq!(describe(&store), ["g:offline"]);
}

#[test]
fn nonsense_indexes_are_survived() {
    let mut store = ready_store();
    store.apply(list_update("everyone", json!([
        { "op": "DELETE", "index": 5 },
        { "op": "INVALIDATE", "range": [0, 99] },
        { "op": "UPDATE", "index": 3, "item": member(1, "x") },
        { "op": "INSERT", "index": 2, "item": member(2, "y") },
        { "op": "SYNC", "range": [10_000_000, 10_000_001], "items": [member(3, "z")] }
    ])));
    // Whatever shape it ended in, nothing blew up or grew without bound.
    assert!(store.member_list(id(GUILD)).unwrap().entries().len() < 100);
}

#[test]
fn member_name_color_comes_from_the_highest_colored_role() {
    let mut ready = ready_json();
    ready["guilds"][0]["roles"] = json!([
        { "id": "100", "name": "@everyone", "permissions": perm(1 << 10) },
        { "id": "200", "name": "low", "color": 0xff0000, "position": 1 },
        { "id": "300", "name": "high", "color": 0x00ff00, "position": 5 },
        { "id": "400", "name": "plain", "color": 0, "position": 9 }
    ]);
    let mut store = Store::new();
    store.apply(event("READY", ready));

    assert_eq!(store.member_color(id(GUILD), &[id(200), id(300)]), Some(0x00ff00));
    assert_eq!(store.member_color(id(GUILD), &[id(400)]), None, "a role with no color doesn't count");
    assert_eq!(store.member_color(id(GUILD), &[]), None);
}

#[test]
fn friend_presences_arrive_with_ready_supplemental() {
    let mut store = ready_store();
    let changes = store.apply(event(
        "READY_SUPPLEMENTAL",
        json!({ "guilds": [], "merged_presences": {
            "friends": [{ "user_id": "2", "status": "idle" }, { "user_id": "3", "status": "offline" }] } }),
    ));
    assert_eq!(store.presence(id(ADA)), Status::Idle);
    assert_eq!(store.presence(id(3)), Status::Offline);
    assert!(changes.presences && changes.dm_list);
}

#[test]
fn group_sizes_come_from_the_top_level_groups_when_headers_carry_none() {
    let mut store = ready_store();
    store.apply(event(
        "GUILD_MEMBER_LIST_UPDATE",
        json!({
            "guild_id": "100", "id": "everyone",
            "groups": [{ "id": "online", "count": 12 }, { "id": "offline", "count": 88 }],
            "ops": [{ "op": "SYNC", "range": [0, 1],
                      "items": [{ "group": { "id": "online" } }, member(10, "a")] }]
        }),
    ));

    let list = store.member_list(id(GUILD)).unwrap();
    assert_eq!(list.group_count("online", 0), 12);
    assert_eq!(list.group_count("offline", 0), 88);
    assert_eq!(list.group_count("unknown", 7), 7, "falls back to the slot's own number");
}

#[test]
fn ready_relationships_resolve_users_from_the_top_level_list() {
    let mut data = ready_json();
    data["relationships"] = json!([
        { "id": "2", "type": 1 },
        { "id": "3", "type": 3, "user": { "id": "3", "username": "bo" } },
        { "id": "4", "type": 5 }
    ]);
    let mut store = Store::new();
    store.apply(event("READY", data));

    let friends = store.relationships(RelationshipKind::Friend);
    assert_eq!(friends.len(), 1);
    assert_eq!(friends[0].display_name(), "Ada");
    // This one carried its user inline.
    let pending = store.relationships(RelationshipKind::IncomingRequest);
    assert_eq!(pending[0].username, "bo");
    // Suggestions are not ties, and we never learned who 4 is anyway.
    assert!(store.relationships(RelationshipKind::Implicit).is_empty());
}

#[test]
fn accepting_a_request_moves_it_to_friends_and_removing_drops_it() {
    let mut store = ready_store();
    let added = store.apply(event(
        "RELATIONSHIP_ADD",
        json!({ "id": "3", "type": 3, "user": { "id": "3", "username": "bo" } }),
    ));
    assert!(added.relationships);
    assert_eq!(store.relationship(id(3)), Some(RelationshipKind::IncomingRequest));

    store.apply(event(
        "RELATIONSHIP_ADD",
        json!({ "id": "3", "type": 1, "user": { "id": "3", "username": "bo" } }),
    ));
    assert_eq!(store.relationships(RelationshipKind::Friend).len(), 1);
    assert!(store.relationships(RelationshipKind::IncomingRequest).is_empty());

    let removed = store.apply(event("RELATIONSHIP_REMOVE", json!({ "id": "3", "type": 1 })));
    assert!(removed.relationships);
    assert_eq!(store.relationship(id(3)), None);
}

#[test]
fn an_open_dm_is_found_by_its_recipient() {
    let mut store = ready_store();
    assert_eq!(store.dm_with(id(ADA)), Some(id(50)));
    assert_eq!(store.dm_with(id(3)), None);

    store.apply(event(
        "CHANNEL_CREATE",
        json!({ "id": "51", "type": 1,
                "recipients": [{ "id": "3", "username": "bo" }] }),
    ));
    assert_eq!(store.dm_with(id(3)), Some(id(51)));
}

#[test]
fn pinning_flips_the_flag_on_a_loaded_message() {
    let mut store = ready_store();
    store.open_channel(id(11));
    store.apply(event(
        "MESSAGE_CREATE",
        json!({ "id": "700", "channel_id": "11", "author": { "id": "2", "username": "ada" },
                "content": "x", "timestamp": "2024-05-01T12:00:00.000000+00:00", "type": 0 }),
    ));
    assert!(!store.timeline(id(11)).unwrap().get(id(700)).unwrap().pinned);

    let changes = store.set_pinned(id(11), id(700), true);
    assert!(changes.messages.contains(&id(11)));
    assert!(store.timeline(id(11)).unwrap().get(id(700)).unwrap().pinned);
    // A message we never loaded changes nothing.
    assert!(store.set_pinned(id(11), id(999), true).is_empty());
}

fn ready_with_settings(settings: Value) -> Store {
    let mut data = ready_json();
    data["user_guild_settings"] = json!({ "entries": settings, "partial": false });
    let mut store = Store::new();
    store.apply(event("READY", data));
    store
}

fn chat(channel: &str, content: &str, mention_everyone: bool) -> Message {
    serde_json::from_value(json!({
        "id": "9000", "channel_id": channel, "guild_id": if channel == "50" { Value::Null } else { json!("100") },
        "author": { "id": "2", "username": "ada" }, "content": content,
        "timestamp": "2024-05-01T12:00:00.000000+00:00", "type": 0,
        "mention_everyone": mention_everyone
    }))
    .unwrap()
}

#[test]
fn a_muted_server_never_shows_unread_but_still_counts_mentions() {
    let store = ready_with_settings(json!([{ "guild_id": "100", "muted": true }]));
    // Channel 11 has unread messages, but the server is muted.
    assert!(!store.unread(id(11)).unread);
    assert!(!store.guild_unread(id(GUILD)).unread);
    assert!(store.guild_muted(id(GUILD)));
}

#[test]
fn an_expired_mute_no_longer_applies() {
    let store = ready_with_settings(json!([{
        "guild_id": "100", "muted": true, "mute_config": { "end_time": "2020-01-01T00:00:00+00:00" }
    }]));
    assert!(!store.guild_muted(id(GUILD)));
    assert!(store.unread(id(11)).unread);
}

#[test]
fn a_channel_override_beats_the_server_setting() {
    let store = ready_with_settings(json!([{
        "guild_id": "100", "message_notifications": 0,
        "channel_overrides": [{ "channel_id": "11", "muted": true }]
    }]));
    let general = store.channel(id(11)).unwrap();
    assert!(store.is_muted(general));
    assert!(!store.is_notable(&chat("11", "hello", false)));
    // Another channel in the same server is not muted and notifies on all.
    assert!(store.is_notable(&chat("15", "hello", false)));
}

#[test]
fn levels_gate_which_messages_notify() {
    let store = ready_with_settings(json!([{ "guild_id": "100", "message_notifications": 1 }]));
    assert!(!store.is_notable(&chat("11", "just chatting", false)));
    assert!(store.is_notable(&chat("11", "@everyone", true)));

    let silent = ready_with_settings(json!([{ "guild_id": "100", "message_notifications": 2 }]));
    assert!(!silent.is_notable(&chat("11", "@everyone", true)));

    // Our own messages never notify, and a DM always does by default.
    assert!(ready_store().is_notable(&chat("50", "hi", false)));
}

#[test]
fn suppressing_everyone_stops_it_counting_as_a_mention() {
    let mut store = ready_with_settings(json!([{ "guild_id": "100", "suppress_everyone": true }]));
    store.open_channel(id(11));
    store.apply(event(
        "MESSAGE_CREATE",
        json!({ "id": "9100", "channel_id": "11", "guild_id": "100",
                "author": { "id": "2", "username": "ada" }, "content": "@everyone",
                "mention_everyone": true,
                "timestamp": "2024-05-01T12:00:00.000000+00:00", "type": 0 }),
    ));
    assert_eq!(store.unread(id(11)).mentions, 0);
}

#[test]
fn settings_updates_replace_one_servers_entry() {
    let mut store = ready_store();
    assert!(!store.guild_muted(id(GUILD)));
    let changes = store.apply(event(
        "USER_GUILD_SETTINGS_UPDATE",
        json!({ "guild_id": "100", "muted": true }),
    ));
    assert!(store.guild_muted(id(GUILD)));
    assert!(changes.guild_list && changes.channel_lists.contains(&id(GUILD)));
}

fn with_reactions() -> Store {
    let mut store = ready_store();
    store.open_channel(id(11));
    store.apply(event(
        "MESSAGE_CREATE",
        json!({ "id": "800", "channel_id": "11", "guild_id": "100",
                "author": { "id": "2", "username": "ada" }, "content": "x",
                "timestamp": "2024-05-01T12:00:00.000000+00:00", "type": 0,
                "reactions": [
                    { "count": 2, "me": false, "emoji": { "name": "👍" } },
                    { "count": 1, "me": true, "emoji": { "name": "🎉" } } ] }),
    ));
    store
}

#[test]
fn clearing_one_emoji_leaves_the_others() {
    let mut store = with_reactions();
    store.apply(event(
        "MESSAGE_REACTION_REMOVE_EMOJI",
        json!({ "channel_id": "11", "message_id": "800", "emoji": { "name": "👍" } }),
    ));
    let reactions = &store.timeline(id(11)).unwrap().get(id(800)).unwrap().reactions;
    assert_eq!(reactions.len(), 1);
    assert_eq!(reactions[0].emoji.name.as_deref(), Some("🎉"));
}

#[test]
fn clearing_all_removes_every_reaction() {
    let mut store = with_reactions();
    let changes = store.apply(event(
        "MESSAGE_REACTION_REMOVE_ALL",
        json!({ "channel_id": "11", "message_id": "800" }),
    ));
    assert!(changes.messages.contains(&id(11)));
    assert!(store.timeline(id(11)).unwrap().get(id(800)).unwrap().reactions.is_empty());
}

#[test]
fn passive_updates_light_up_channels_in_big_servers() {
    let mut store = ready_store();
    assert!(!store.unread(id(15)).unread);
    let changes = store.apply(event(
        "PASSIVE_UPDATE_V2",
        json!({ "guild_id": "100", "updated_channels": [{ "id": "15", "last_message_id": "9999" }] }),
    ));
    assert!(changes.guild_list);
    assert_eq!(store.channel(id(15)).unwrap().last_message_id, Some(id(9999)));
    // An older id never moves the marker back.
    store.apply(event(
        "PASSIVE_UPDATE_V2",
        json!({ "guild_id": "100", "updated_channels": [{ "id": "15", "last_message_id": "5" }] }),
    ));
    assert_eq!(store.channel(id(15)).unwrap().last_message_id, Some(id(9999)));
}

#[test]
fn our_own_profile_updates_and_group_members_come_and_go() {
    let mut store = ready_store();
    store.apply(event("USER_UPDATE", json!({ "id": "1", "username": "me", "global_name": "New Me" })));
    assert_eq!(store.me().unwrap().display_name(), "New Me");

    store.apply(event(
        "CHANNEL_RECIPIENT_ADD",
        json!({ "channel_id": "50", "user": { "id": "7", "username": "cy" } }),
    ));
    assert_eq!(store.channel(id(50)).unwrap().recipients.len(), 2);
    store.apply(event(
        "CHANNEL_RECIPIENT_REMOVE",
        json!({ "channel_id": "50", "user": { "id": "7", "username": "cy" } }),
    ));
    assert_eq!(store.channel(id(50)).unwrap().recipients.len(), 1);
}

fn thread(id: &str, parent: &str, archived: bool) -> Value {
    json!({ "id": id, "type": 11, "guild_id": "100", "parent_id": parent, "name": format!("t{id}"),
            "last_message_id": id, "thread_metadata": { "archived": archived, "locked": false } })
}

#[test]
fn threads_hang_off_their_parent_and_hide_when_archived() {
    let mut store = ready_store();
    store.apply(event("THREAD_CREATE", thread("301", "11", false)));
    store.apply(event("THREAD_CREATE", thread("302", "11", false)));
    store.apply(event("THREAD_CREATE", thread("303", "15", false)));

    let open: Vec<_> = store.open_threads(id(11)).iter().map(|t| t.id).collect();
    assert_eq!(open, vec![id(302), id(301)], "newest activity first");

    store.apply(event("THREAD_UPDATE", thread("301", "11", true)));
    assert_eq!(store.open_threads(id(11)).len(), 1);
    // Threads never appear in the server's top level channel list.
    let listed: usize = store.channel_groups(id(GUILD)).iter().map(|g| g.channels.len()).sum();
    assert!(store.channel_groups(id(GUILD)).iter().flat_map(|g| &g.channels).all(|c| !c.kind.is_thread()));
    assert!(listed > 0);

    store.apply(event("THREAD_DELETE", json!({ "id": "302", "guild_id": "100", "parent_id": "11", "type": 11 })));
    assert!(store.open_threads(id(11)).is_empty());
}

#[test]
fn joining_is_tracked_and_survives_thread_updates() {
    let mut store = ready_store();
    let mut created = thread("301", "11", false);
    created["member"] = json!({ "flags": 0 });
    store.apply(event("THREAD_CREATE", created));
    assert_eq!(store.joined_threads(id(11)).len(), 1);

    // An update that omits `member` must not make us leave.
    store.apply(event("THREAD_UPDATE", thread("301", "11", false)));
    assert_eq!(store.joined_threads(id(11)).len(), 1);

    store.apply(event(
        "THREAD_MEMBERS_UPDATE",
        json!({ "id": "301", "guild_id": "100", "member_count": 3, "removed_member_ids": ["1"] }),
    ));
    assert!(store.joined_threads(id(11)).is_empty());
    assert_eq!(store.channel(id(301)).unwrap().member_count, Some(3));

    store.apply(event("THREAD_MEMBER_UPDATE", json!({ "id": "301", "guild_id": "100", "flags": 0 })));
    assert_eq!(store.joined_threads(id(11)).len(), 1);
}

#[test]
fn a_thread_list_sync_replaces_the_threads_it_covers() {
    let mut store = ready_store();
    store.apply(event("THREAD_CREATE", thread("301", "11", false)));
    store.apply(event("THREAD_CREATE", thread("303", "15", false)));

    store.apply(event(
        "THREAD_LIST_SYNC",
        json!({ "guild_id": "100", "channel_ids": ["11"],
                "threads": [thread("305", "11", false)],
                "members": [{ "id": "305", "user_id": "1", "flags": 0 }] }),
    ));
    let under_11: Vec<_> = store.open_threads(id(11)).iter().map(|t| t.id).collect();
    assert_eq!(under_11, vec![id(305)], "301 was not listed, so it is gone");
    assert_eq!(store.joined_threads(id(11)).len(), 1);
    // A parent the sync didn't cover keeps its threads.
    assert_eq!(store.open_threads(id(15)).len(), 1);
}

#[test]
fn poll_votes_arrive_live_and_our_own_apply_at_once() {
    let mut store = ready_store();
    store.open_channel(id(11));
    store.apply(event(
        "MESSAGE_CREATE",
        json!({ "id": "820", "channel_id": "11", "guild_id": "100",
                "author": { "id": "2", "username": "ada" }, "content": "",
                "timestamp": "2024-05-01T12:00:00.000000+00:00", "type": 0,
                "poll": { "question": { "text": "Q" }, "answers": [
                    { "answer_id": 1, "poll_media": { "text": "A" } },
                    { "answer_id": 2, "poll_media": { "text": "B" } } ] } }),
    ));
    let poll = |s: &Store| s.timeline(id(11)).unwrap().get(id(820)).unwrap().poll.clone().unwrap();

    store.apply(event(
        "MESSAGE_POLL_VOTE_ADD",
        json!({ "user_id": "2", "channel_id": "11", "message_id": "820", "answer_id": 1 }),
    ));
    assert_eq!(poll(&store).votes(1).count, 1);

    store.set_my_poll_votes(id(11), id(820), &[2]);
    assert!(poll(&store).votes(2).me);
    store.apply(event(
        "MESSAGE_POLL_VOTE_REMOVE",
        json!({ "user_id": "1", "channel_id": "11", "message_id": "820", "answer_id": 2 }),
    ));
    assert!(!poll(&store).votes(2).me);
    assert_eq!(poll(&store).votes(2).count, 0);
}

fn window_message(id: &str, content: &str) -> Value {
    json!({ "id": id, "channel_id": "11", "guild_id": "100",
            "author": { "id": "2", "username": "ada" }, "content": content,
            "timestamp": "2024-05-01T12:00:00.000000+00:00", "type": 0 })
}

#[test]
fn a_jumped_to_window_replaces_the_timeline_and_ignores_live_messages() {
    let mut store = ready_store();
    store.open_channel(id(11));
    store.apply(event("MESSAGE_CREATE", window_message("900", "newest")));

    let page: Vec<Message> = [("100", "old one"), ("101", "old two")]
        .iter()
        .map(|(i, c)| serde_json::from_value(window_message(i, c)).unwrap())
        .collect();
    let changes = store.show_window(id(11), page);
    assert!(changes.messages.contains(&id(11)));

    let timeline = store.timeline(id(11)).unwrap();
    assert!(timeline.is_detached());
    assert!(timeline.get(id(900)).is_none(), "the newest message is not part of this window");
    assert_eq!(timeline.messages().len(), 2);

    // A live message must not land beside messages from long ago.
    store.apply(event("MESSAGE_CREATE", window_message("901", "live")));
    assert!(store.timeline(id(11)).unwrap().get(id(901)).is_none());
    // It still counts as unread, which does not depend on the timeline.
    assert!(store.unread(id(11)).unread);
}

#[test]
fn returning_to_the_present_starts_the_timeline_fresh() {
    let mut store = ready_store();
    store.show_window(id(11), vec![serde_json::from_value(window_message("100", "old")).unwrap()]);
    store.return_to_present(id(11));
    let timeline = store.timeline(id(11)).unwrap();
    assert!(!timeline.is_detached() && timeline.messages().is_empty());
    store.apply(event("MESSAGE_CREATE", window_message("902", "live again")));
    assert!(store.timeline(id(11)).unwrap().get(id(902)).is_some());
}

#[test]
fn voice_channels_list_who_joined_and_follow_them_around() {
    let mut store = ready_store();
    let join = |user: u64, channel: Value| {
        event(
            "VOICE_STATE_UPDATE",
            json!({ "user_id": user.to_string(), "guild_id": "100", "channel_id": channel,
                    "self_mute": true, "member": { "user": { "id": user.to_string(), "username": "newcomer" } } }),
        )
    };

    let changes = store.apply(join(7, json!("13")));
    assert!(changes.channel_lists.contains(&id(100)), "the channel list redraws");
    assert_eq!(store.voice_in(id(13)).len(), 1);
    assert!(store.voice_in(id(13))[0].muted());
    assert_eq!(store.user(id(7)).map(|u| u.username.as_str()), Some("newcomer"));

    // Moving to another channel, then leaving, empties the first.
    store.apply(join(7, json!("12")));
    assert!(store.voice_in(id(13)).is_empty() && store.voice_in(id(12)).len() == 1);
    store.apply(join(7, Value::Null));
    assert!(store.voice_in(id(12)).is_empty());
}

#[test]
fn a_guild_payload_seeds_the_voice_lists() {
    let mut data = ready_json();
    data["guilds"][0]["voice_states"] = json!([{ "user_id": "2", "channel_id": "13" }]);
    let mut store = Store::new();
    store.apply(event("READY", data));
    assert_eq!(store.voice_in(id(13)).len(), 1);
}
