use oxicord_model::Message;
use oxicord_store::Store;

/// Longest body we hand to the system, which truncates on its own anyway but
/// not always gracefully.
const MAX_BODY: usize = 200;

/// The title and text of the toast for `message`.
pub fn compose(store: &Store, message: &Message) -> (String, String) {
    let author = message.author.display_name();
    let channel = store.channel(message.channel_id);

    let title = match channel {
        Some(c) if c.guild_id.is_some() => {
            let guild = c.guild_id.and_then(|g| store.guild(g)).map(|g| g.name.as_str());
            match (c.name.as_deref(), guild) {
                (Some(channel), Some(guild)) => format!("{author} (#{channel}, {guild})"),
                (Some(channel), None) => format!("{author} (#{channel})"),
                _ => author.to_owned(),
            }
        }
        // In a group DM the sender alone doesn't say which group.
        Some(c) if c.kind == oxicord_model::ChannelType::GroupDm => {
            format!("{author} ({})", store.dm_name(c))
        }
        _ => author.to_owned(),
    };

    let body = if message.content.trim().is_empty() {
        match message.attachments.len() {
            0 => "Sent a message".to_owned(),
            1 => "Sent an attachment".to_owned(),
            n => format!("Sent {n} attachments"),
        }
    } else {
        truncate(message.content.trim())
    };
    (title, body)
}

fn truncate(text: &str) -> String {
    match text.char_indices().nth(MAX_BODY) {
        Some((end, _)) => format!("{}...", &text[..end]),
        None => text.to_owned(),
    }
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
                "users": [{ "id": "2", "username": "ada", "global_name": "Ada" }],
                "guilds": [{ "id": "100", "properties": { "name": "home" },
                    "channels": [{ "id": "11", "type": 0, "name": "general" }] }],
                "private_channels": [{ "id": "50", "type": 1, "recipient_ids": ["2"] }]
            }),
        )
        .unwrap();
        store.apply(ready);
        store
    }

    fn message(channel: &str, content: &str) -> Message {
        serde_json::from_value(json!({
            "id": "9", "channel_id": channel,
            "author": { "id": "2", "username": "ada", "global_name": "Ada" },
            "content": content, "timestamp": "2024-05-01T12:00:00.000000+00:00", "type": 0
        }))
        .unwrap()
    }

    #[test]
    fn a_dm_is_titled_with_just_the_sender() {
        let (title, body) = compose(&store(), &message("50", " hi "));
        assert_eq!((title.as_str(), body.as_str()), ("Ada", "hi"));
    }

    #[test]
    fn a_server_message_names_the_channel_and_server() {
        let (title, _) = compose(&store(), &message("11", "hey"));
        assert_eq!(title, "Ada (#general, home)");
    }

    #[test]
    fn empty_text_and_long_text_are_handled() {
        let (_, body) = compose(&store(), &message("50", ""));
        assert_eq!(body, "Sent a message");
        let (_, body) = compose(&store(), &message("50", &"x".repeat(500)));
        assert_eq!(body.chars().count(), MAX_BODY + 3);
    }
}
