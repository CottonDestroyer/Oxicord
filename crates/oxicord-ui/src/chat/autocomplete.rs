//! Suggestions while typing `@name`, `#channel` or `:emoji`. The logic is
//! kept apart from the views so it can be tested with plain values.

use std::collections::HashSet;

use oxicord_model::{ChannelType, Snowflake};
use oxicord_store::{Entry, Store};

/// Longest list offered at once.
const MAX_SUGGESTIONS: usize = 8;
/// A colon only starts an emoji search after this many letters, so ordinary
/// punctuation like "note: ..." never pops anything up.
const MIN_EMOJI_QUERY: usize = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Mention,
    Channel,
    Emoji,
}

/// The word being typed at the end of the text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Trigger {
    pub kind: Kind,
    pub query: String,
    /// Byte offset of the trigger character, where a pick starts replacing.
    pub start: usize,
}

/// Finds the `@`, `#` or `:` word the cursor sits at the end of.
pub fn trigger(text: &str) -> Option<Trigger> {
    let start = text
        .rfind(|c: char| c.is_whitespace())
        .map_or(0, |i| i + text[i..].chars().next().map_or(1, char::len_utf8));
    let word = &text[start..];
    let mut chars = word.chars();
    let kind = match chars.next()? {
        '@' => Kind::Mention,
        '#' => Kind::Channel,
        ':' => Kind::Emoji,
        _ => return None,
    };
    let query: String = chars.collect();
    // A finished `:smile:` is not a search any more.
    if kind == Kind::Emoji && (query.chars().count() < MIN_EMOJI_QUERY || query.contains(':')) {
        return None;
    }
    Some(Trigger { kind, query, start })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Suggestion {
    /// What the list shows.
    pub label: String,
    /// A second, quieter line such as a username.
    pub detail: String,
    /// What replaces the typed word when picked.
    pub insert: String,
}

/// Ranks names against a query: those that start with it first, then those
/// that merely contain it.
fn rank(name: &str, query: &str) -> Option<u8> {
    let (name, query) = (name.to_lowercase(), query.to_lowercase());
    if query.is_empty() || name.starts_with(&query) {
        Some(0)
    } else if name.contains(&query) {
        Some(1)
    } else {
        None
    }
}

pub fn suggest(
    store: &Store,
    guild: Option<Snowflake>,
    channel: Snowflake,
    trigger: &Trigger,
) -> Vec<Suggestion> {
    let mut scored: Vec<(u8, Suggestion)> = match trigger.kind {
        Kind::Mention => mentions(store, guild, channel, &trigger.query),
        Kind::Channel => channels(store, guild, &trigger.query),
        Kind::Emoji => emoji(store, guild, &trigger.query),
    };
    scored.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.label.cmp(&b.1.label)));
    scored.truncate(MAX_SUGGESTIONS);
    scored.into_iter().map(|(_, s)| s).collect()
}

fn mentions(
    store: &Store,
    guild: Option<Snowflake>,
    channel: Snowflake,
    query: &str,
) -> Vec<(u8, Suggestion)> {
    let mut seen: HashSet<Snowflake> = HashSet::new();
    let mut people: Vec<(Snowflake, Option<String>)> = Vec::new();

    // The people in this conversation: its member list, who spoke recently,
    // or the other side of a direct message.
    if let Some(list) = guild.and_then(|g| store.member_list(g)) {
        for entry in list.entries().iter().flatten() {
            if let Entry::Member { user, nick, .. } = entry {
                people.push((*user, nick.clone()));
            }
        }
    }
    if let Some(timeline) = store.timeline(channel) {
        for message in timeline.messages().iter().rev().take(200) {
            people.push((message.author.id, None));
        }
    }
    if let Some(c) = store.channel(channel) {
        people.extend(c.recipients.iter().map(|u| (u.id, None)));
    }

    let mut out = Vec::new();
    for (id, nick) in people {
        if !seen.insert(id) {
            continue;
        }
        let Some(user) = store.user(id) else { continue };
        let shown = nick.filter(|n| !n.is_empty()).unwrap_or_else(|| user.display_name().to_owned());
        let best = [rank(&shown, query), rank(&user.username, query)].into_iter().flatten().min();
        if let Some(score) = best {
            out.push((
                score,
                Suggestion {
                    label: shown,
                    detail: user.username.clone(),
                    insert: format!("<@{id}>"),
                },
            ));
        }
    }

    if guild.is_some() {
        for special in ["everyone", "here"] {
            if let Some(score) = rank(special, query) {
                out.push((
                    score + 1,
                    Suggestion {
                        label: format!("@{special}"),
                        detail: "Notify everyone in the channel".into(),
                        insert: format!("@{special}"),
                    },
                ));
            }
        }
        if let Some(g) = guild.and_then(|g| store.guild(g)) {
            for role in g.roles.iter().filter(|r| r.mentionable && r.id != g.id) {
                if let Some(score) = rank(&role.name, query) {
                    out.push((
                        score,
                        Suggestion {
                            label: format!("@{}", role.name),
                            detail: "Role".into(),
                            insert: format!("<@&{}>", role.id),
                        },
                    ));
                }
            }
        }
    }
    out
}

fn channels(store: &Store, guild: Option<Snowflake>, query: &str) -> Vec<(u8, Suggestion)> {
    let Some(guild) = guild else { return Vec::new() };
    store
        .channel_groups(guild)
        .into_iter()
        .flat_map(|group| group.channels)
        .filter(|c| !matches!(c.kind, ChannelType::GuildCategory))
        .filter_map(|c| {
            let name = c.name.as_deref()?;
            Some((
                rank(name, query)?,
                Suggestion {
                    label: format!("#{name}"),
                    detail: String::new(),
                    insert: format!("<#{}>", c.id),
                },
            ))
        })
        .collect()
}

fn emoji(store: &Store, guild: Option<Snowflake>, query: &str) -> Vec<(u8, Suggestion)> {
    let mut out = Vec::new();
    if let Some(g) = guild.and_then(|g| store.guild(g)) {
        for e in &g.emojis {
            let (Some(id), Some(name)) = (e.id, e.name.as_deref()) else { continue };
            if let Some(score) = rank(name, query) {
                out.push((
                    score,
                    Suggestion {
                        label: format!(":{name}:"),
                        detail: "Server emoji".into(),
                        insert: format!("<{}:{name}:{id}>", if e.animated { "a" } else { "" }),
                    },
                ));
            }
        }
    }
    for e in emojis::iter().filter(|e| e.unicode_version() <= emojis::UnicodeVersion::new(13, 0)) {
        if let Some(code) = e.shortcodes().find(|s| rank(s, query).is_some()) {
            out.push((
                rank(code, query).unwrap_or(1) + 1,
                Suggestion {
                    label: format!("{} :{code}:", e.as_str()),
                    detail: String::new(),
                    insert: e.as_str().to_owned(),
                },
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use oxicord_model::gateway::DispatchEvent;
    use serde_json::json;

    use super::*;

    fn store() -> Store {
        let mut store = Store::new();
        let ready = DispatchEvent::parse(
            "READY",
            json!({
                "user": { "id": "1", "username": "me" }, "session_id": "s",
                "users": [
                    { "id": "2", "username": "ada", "global_name": "Ada Lovelace" },
                    { "id": "3", "username": "bob" }
                ],
                "guilds": [{ "id": "100", "properties": { "name": "g" },
                    "roles": [{ "id": "100", "name": "@everyone", "permissions": "1024" },
                              { "id": "200", "name": "Moderators", "mentionable": true }],
                    "emojis": [{ "id": "900", "name": "party_blob" }],
                    "channels": [
                        { "id": "10", "type": 0, "name": "general", "position": 0 },
                        { "id": "11", "type": 0, "name": "random", "position": 1 }] }],
                "private_channels": [{ "id": "50", "type": 1, "recipient_ids": ["2"] }]
            }),
        )
        .unwrap();
        store.apply(ready);
        store
    }

    #[test]
    fn finds_the_word_being_typed() {
        assert_eq!(trigger("hello @ad"), Some(Trigger { kind: Kind::Mention, query: "ad".into(), start: 6 }));
        assert_eq!(trigger("#gen"), Some(Trigger { kind: Kind::Channel, query: "gen".into(), start: 0 }));
        assert_eq!(trigger("so :smi").unwrap().kind, Kind::Emoji);
        assert_eq!(trigger("@").unwrap().query, "");
    }

    #[test]
    fn ordinary_text_does_not_trigger() {
        assert_eq!(trigger("hello"), None);
        assert_eq!(trigger("hello @ada "), None, "a space ends the word");
        assert_eq!(trigger("note:"), None);
        assert_eq!(trigger("a :s"), None, "one letter is too short for emoji");
        assert_eq!(trigger(":smile:"), None, "finished shortcodes are done");
        assert_eq!(trigger(""), None);
    }

    #[test]
    fn mentions_match_display_name_and_username_in_a_dm() {
        let store = store();
        let t = trigger("@lov").unwrap();
        let found = suggest(&store, None, Snowflake(50), &t);
        assert_eq!(found[0].label, "Ada Lovelace");
        assert_eq!(found[0].insert, "<@2>");
        // Username matches too.
        let by_username = suggest(&store, None, Snowflake(50), &trigger("@ad").unwrap());
        assert_eq!(by_username[0].detail, "ada");
    }

    #[test]
    fn servers_add_everyone_here_and_mentionable_roles() {
        let store = store();
        let found = suggest(&store, Some(Snowflake(100)), Snowflake(10), &trigger("@").unwrap());
        let labels: Vec<_> = found.iter().map(|s| s.label.as_str()).collect();
        assert!(labels.contains(&"@everyone") && labels.contains(&"@here"));
        let role = found.iter().find(|s| s.label == "@Moderators").unwrap();
        assert_eq!(role.insert, "<@&200>");
    }

    #[test]
    fn channels_insert_their_id() {
        let store = store();
        let found = suggest(&store, Some(Snowflake(100)), Snowflake(10), &trigger("#ran").unwrap());
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].insert, "<#11>");
    }

    #[test]
    fn emoji_offer_server_emoji_first_then_unicode() {
        let store = store();
        let found = suggest(&store, Some(Snowflake(100)), Snowflake(10), &trigger(":party").unwrap());
        assert_eq!(found[0].insert, "<:party_blob:900>");
        assert!(found.iter().any(|s| s.insert == "🥳"), "{found:?}");
    }

    #[test]
    fn prefix_matches_outrank_substring_matches() {
        assert_eq!(rank("random", "ran"), Some(0));
        assert_eq!(rank("brandon", "ran"), Some(1));
        assert_eq!(rank("zzz", "ran"), None);
    }
}
