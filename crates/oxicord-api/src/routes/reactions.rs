use oxicord_model::{Emoji, Snowflake};
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use reqwest::Method;

use crate::{ApiClient, ApiError};

/// Path form of an emoji: the character itself for unicode, `name:id` for
/// custom ones, percent encoded either way.
pub fn emoji_path_segment(emoji: &Emoji) -> String {
    let name = emoji.name.as_deref().unwrap_or_default();
    let raw = match emoji.id {
        Some(id) => format!("{name}:{id}"),
        None => name.to_owned(),
    };
    utf8_percent_encode(&raw, NON_ALPHANUMERIC).to_string()
}

impl ApiClient {
    pub async fn add_reaction(
        &self,
        channel: Snowflake,
        message: Snowflake,
        emoji: &Emoji,
    ) -> Result<(), ApiError> {
        let path = format!(
            "/channels/{channel}/messages/{message}/reactions/{}/@me?location=Message&type=0",
            emoji_path_segment(emoji)
        );
        let route = format!("PUT /channels/{channel}/messages/:id/reactions/:emoji/:user");
        self.empty(Method::PUT, &path, &route, None).await
    }

    /// The first `limit` people who reacted with `emoji`.
    pub async fn reactors(
        &self,
        channel: Snowflake,
        message: Snowflake,
        emoji: &Emoji,
        limit: u8,
    ) -> Result<Vec<oxicord_model::User>, ApiError> {
        let path = format!(
            "/channels/{channel}/messages/{message}/reactions/{}?limit={limit}&type=0",
            emoji_path_segment(emoji)
        );
        let route = format!("GET /channels/{channel}/messages/:id/reactions/:emoji");
        self.json(Method::GET, &path, &route, None).await
    }

    pub async fn remove_reaction(
        &self,
        channel: Snowflake,
        message: Snowflake,
        emoji: &Emoji,
    ) -> Result<(), ApiError> {
        let path = format!(
            "/channels/{channel}/messages/{message}/reactions/{}/@me?location=Message&type=0",
            emoji_path_segment(emoji)
        );
        let route = format!("DELETE /channels/{channel}/messages/:id/reactions/:emoji/:user");
        self.empty(Method::DELETE, &path, &route, None).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_unicode_emoji() {
        let e = Emoji {
            name: Some("👍".into()),
            ..Default::default()
        };
        assert_eq!(emoji_path_segment(&e), "%F0%9F%91%8D");
    }

    #[test]
    fn encodes_custom_emoji_with_id() {
        let e = Emoji {
            id: Some(Snowflake(42)),
            name: Some("party".into()),
            animated: false,
        };
        assert_eq!(emoji_path_segment(&e), "party%3A42");
    }
}
