//! Turns what was typed in the search box into a [`SearchQuery`]. Words
//! stay as text, while `from:`, `in:`, `mentions:` and `has:` become filters.
//! Names are resolved to ids here because the server wants ids.

use oxicord_model::{SearchQuery, Snowflake};
use oxicord_store::Store;

const HAS: [&str; 7] = ["image", "video", "file", "link", "embed", "sound", "sticker"];

pub fn parse(store: &Store, guild: Option<Snowflake>, channel: Snowflake, text: &str) -> SearchQuery {
    let mut query = SearchQuery::default();
    let mut words = Vec::new();

    for token in text.split_whitespace() {
        let Some((key, value)) = token.split_once(':') else {
            words.push(token);
            continue;
        };
        let value = value.trim();
        let handled = match key.to_lowercase().as_str() {
            "from" => resolve_user(store, guild, channel, value).map(|id| query.authors.push(id)),
            "mentions" => resolve_user(store, guild, channel, value).map(|id| query.mentions.push(id)),
            "in" => resolve_channel(store, guild, value).map(|id| query.channels.push(id)),
            "has" => {
                let kind = value.to_lowercase();
                HAS.contains(&kind.as_str()).then(|| query.has.push(kind))
            }
            _ => None,
        };
        // A filter we can't resolve stays in the text, so the search still
        // looks for what was typed rather than silently ignoring it.
        if handled.is_none() {
            words.push(token);
        }
    }
    query.content = words.join(" ");
    query
}

fn numeric_id(value: &str) -> Option<Snowflake> {
    let digits = value.trim_start_matches("<@").trim_start_matches("<#").trim_end_matches('>');
    digits.parse::<u64>().ok().filter(|n| *n > 1_000_000).map(Snowflake)
}

fn resolve_user(store: &Store, guild: Option<Snowflake>, channel: Snowflake, value: &str) -> Option<Snowflake> {
    if let Some(id) = numeric_id(value) {
        return Some(id);
    }
    let wanted = value.trim_start_matches('@').to_lowercase();
    if wanted.is_empty() {
        return None;
    }
    // People we know about in this conversation come first.
    let mut candidates: Vec<Snowflake> = Vec::new();
    if let Some(list) = guild.and_then(|g| store.member_list(g)) {
        for entry in list.entries().iter().flatten() {
            if let oxicord_store::Entry::Member { user, .. } = entry {
                candidates.push(*user);
            }
        }
    }
    if let Some(timeline) = store.timeline(channel) {
        candidates.extend(timeline.messages().iter().map(|m| m.author.id));
    }
    if let Some(c) = store.channel(channel) {
        candidates.extend(c.recipients.iter().map(|u| u.id));
    }

    let matches = |exact: bool| {
        candidates.iter().copied().find(|id| {
            store.user(*id).is_some_and(|u| {
                let names = [u.username.to_lowercase(), u.display_name().to_lowercase()];
                names.iter().any(|n| if exact { *n == wanted } else { n.starts_with(&wanted) })
            })
        })
    };
    matches(true).or_else(|| matches(false))
}

fn resolve_channel(store: &Store, guild: Option<Snowflake>, value: &str) -> Option<Snowflake> {
    if let Some(id) = numeric_id(value) {
        return Some(id);
    }
    let wanted = value.trim_start_matches('#').to_lowercase();
    store
        .channel_groups(guild?)
        .into_iter()
        .flat_map(|g| g.channels)
        .find(|c| c.name.as_deref().is_some_and(|n| n.to_lowercase() == wanted))
        .map(|c| c.id)
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
                "users": [{ "id": "2000000", "username": "ada", "global_name": "Ada Lovelace" }],
                "guilds": [{ "id": "100", "properties": { "name": "g" },
                    "roles": [{ "id": "100", "name": "@everyone", "permissions": "1024" }],
                    "channels": [{ "id": "3000000", "type": 0, "name": "general", "position": 0 }] }],
                "private_channels": [{ "id": "4000000", "type": 1, "recipient_ids": ["2000000"] }]
            }),
        )
        .unwrap();
        store.apply(ready);
        store
    }

    #[test]
    fn plain_words_stay_as_text() {
        let q = parse(&store(), None, Snowflake(4_000_000), "hello there");
        assert_eq!(q.content, "hello there");
        assert!(q.authors.is_empty() && q.has.is_empty());
    }

    #[test]
    fn from_resolves_a_name_in_the_conversation() {
        let q = parse(&store(), None, Snowflake(4_000_000), "from:ada budget");
        assert_eq!(q.authors, vec![Snowflake(2_000_000)]);
        assert_eq!(q.content, "budget");
        // A prefix of the display name works too.
        let q = parse(&store(), None, Snowflake(4_000_000), "from:ada");
        assert_eq!(q.authors.len(), 1);
    }

    #[test]
    fn in_resolves_a_channel_name_and_has_checks_its_kind() {
        let q = parse(&store(), Some(Snowflake(100)), Snowflake(3_000_000), "in:#general has:image cats");
        assert_eq!(q.channels, vec![Snowflake(3_000_000)]);
        assert_eq!(q.has, vec!["image".to_owned()]);
        assert_eq!(q.content, "cats");
    }

    #[test]
    fn ids_and_mentions_are_taken_as_they_are() {
        let q = parse(&store(), None, Snowflake(4_000_000), "from:<@5555555> mentions:6666666");
        assert_eq!(q.authors, vec![Snowflake(5_555_555)]);
        assert_eq!(q.mentions, vec![Snowflake(6_666_666)]);
    }

    #[test]
    fn unknown_filters_remain_in_the_text() {
        let q = parse(&store(), None, Snowflake(4_000_000), "from:nobody has:banana note:x");
        assert!(q.authors.is_empty() && q.has.is_empty());
        assert_eq!(q.content, "from:nobody has:banana note:x");
    }
}
