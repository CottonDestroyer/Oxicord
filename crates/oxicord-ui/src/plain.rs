//! Message text for lists that show a message as one flat line, such as the
//! inbox and search results: mentions and emoji become readable names.

use oxicord_model::{Message, Snowflake};
use oxicord_store::Store;

/// `content` with `<@id>`, `<@&id>`, `<#id>` and `<:name:id>` replaced by what
/// they stand for.
pub fn plain_text(message: &Message, store: &Store) -> String {
    let guild = store.channel(message.channel_id).and_then(|c| c.guild_id);
    expand(&message.content, |token| match token {
        Token::User(id) => message
            .mentions
            .iter()
            .find(|u| u.id == id)
            .or_else(|| store.user(id))
            .map(|u| format!("@{}", u.display_name())),
        Token::Role(id) => guild
            .and_then(|g| store.guild(g))
            .and_then(|g| g.roles.iter().find(|r| r.id == id))
            .map(|r| format!("@{}", r.name)),
        Token::Channel(id) => store.channel(id).and_then(|c| c.name.as_deref()).map(|n| format!("#{n}")),
        Token::Emoji(name) => Some(format!(":{name}:")),
    })
}

enum Token<'a> {
    User(Snowflake),
    Role(Snowflake),
    Channel(Snowflake),
    Emoji(&'a str),
}

/// Walks the text, handing every `<...>` token to `name`. A token it cannot
/// resolve is kept as typed.
fn expand<'a>(text: &'a str, name: impl Fn(Token<'a>) -> Option<String>) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(open) = rest.find('<') {
        out.push_str(&rest[..open]);
        let tail = &rest[open..];
        let resolved = tail.find('>').and_then(|close| {
            let replacement = parse(&tail[1..close]).and_then(&name)?;
            Some((replacement, close))
        });
        match resolved {
            Some((replacement, close)) => {
                out.push_str(&replacement);
                rest = &tail[close + 1..];
            }
            // Only the bracket is consumed, so a real token after a stray
            // one is still found.
            None => {
                out.push('<');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn parse(inner: &str) -> Option<Token<'_>> {
    let id = |s: &str| s.parse::<u64>().ok().map(Snowflake);
    if let Some(rest) = inner.strip_prefix("@&") {
        id(rest).map(Token::Role)
    } else if let Some(rest) = inner.strip_prefix("@!").or_else(|| inner.strip_prefix('@')) {
        id(rest).map(Token::User)
    } else if let Some(rest) = inner.strip_prefix('#') {
        id(rest).map(Token::Channel)
    } else {
        let rest = inner.strip_prefix("a:").or_else(|| inner.strip_prefix(':'))?;
        let (name, digits) = rest.trim_start_matches(':').rsplit_once(':')?;
        id(digits)?;
        Some(Token::Emoji(name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolve(token: Token<'_>) -> Option<String> {
        match token {
            Token::User(Snowflake(1)) => Some("@Ada".into()),
            Token::Channel(Snowflake(5)) => Some("#general".into()),
            Token::Emoji(name) => Some(format!(":{name}:")),
            _ => None,
        }
    }

    #[test]
    fn mentions_and_emoji_become_names() {
        assert_eq!(expand("hi <@1> in <#5> <:wave:99> <a:spin:7>", resolve), "hi @Ada in #general :wave: :spin:");
    }

    #[test]
    fn nicknamed_mentions_use_the_same_form() {
        assert_eq!(expand("<@!1>", resolve), "@Ada");
    }

    #[test]
    fn what_cannot_be_resolved_stays_as_typed() {
        assert_eq!(expand("<@2> <not a token> 1 < 2", resolve), "<@2> <not a token> 1 < 2");
        assert_eq!(expand("a < b <@1>", resolve), "a < b @Ada");
    }
}
