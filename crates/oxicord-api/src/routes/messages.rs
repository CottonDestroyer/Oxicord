use oxicord_model::{Message, Snowflake};
use reqwest::Method;
use serde::Serialize;
use serde_json::json;

use crate::{ApiClient, ApiError};

/// Which slice of history to fetch. Set at most one of the cursors.
#[derive(Clone, Copy, Debug)]
pub struct MessagePage {
    pub before: Option<Snowflake>,
    pub after: Option<Snowflake>,
    pub around: Option<Snowflake>,
    pub limit: u8,
}

impl Default for MessagePage {
    fn default() -> Self {
        Self {
            before: None,
            after: None,
            around: None,
            limit: 50,
        }
    }
}

impl MessagePage {
    fn query(&self) -> String {
        // Discord rejects limits outside 1..=100.
        let mut q = format!("limit={}", self.limit.clamp(1, 100));
        for (key, cursor) in [
            ("before", self.before),
            ("after", self.after),
            ("around", self.around),
        ] {
            if let Some(id) = cursor {
                q.push_str(&format!("&{key}={id}"));
            }
        }
        q
    }
}

/// The message a new message answers.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct ReplyTarget {
    pub message_id: Snowflake,
    pub channel_id: Snowflake,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guild_id: Option<Snowflake>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct SendMessage {
    pub content: String,
    /// Round-trips on the created message so the UI can swap its pending
    /// copy for the real one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nonce: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_reference: Option<ReplyTarget>,
}

impl ApiClient {
    pub async fn messages(
        &self,
        channel: Snowflake,
        page: MessagePage,
    ) -> Result<Vec<Message>, ApiError> {
        self.json(
            Method::GET,
            &format!("/channels/{channel}/messages?{}", page.query()),
            &format!("GET /channels/{channel}/messages"),
            None,
        )
        .await
    }

    pub async fn send_message(
        &self,
        channel: Snowflake,
        message: &SendMessage,
    ) -> Result<Message, ApiError> {
        let body = serde_json::to_value(message)?;
        self.json(
            Method::POST,
            &format!("/channels/{channel}/messages"),
            &format!("POST /channels/{channel}/messages"),
            Some(&body),
        )
        .await
    }

    pub async fn edit_message(
        &self,
        channel: Snowflake,
        message: Snowflake,
        content: &str,
    ) -> Result<Message, ApiError> {
        self.json(
            Method::PATCH,
            &format!("/channels/{channel}/messages/{message}"),
            &format!("PATCH /channels/{channel}/messages/:id"),
            Some(&json!({ "content": content })),
        )
        .await
    }

    pub async fn delete_message(
        &self,
        channel: Snowflake,
        message: Snowflake,
    ) -> Result<(), ApiError> {
        self.empty(
            Method::DELETE,
            &format!("/channels/{channel}/messages/{message}"),
            &format!("DELETE /channels/{channel}/messages/:id"),
            None,
        )
        .await
    }

    /// Marks everything up to `message` as read.
    pub async fn ack_message(
        &self,
        channel: Snowflake,
        message: Snowflake,
    ) -> Result<(), ApiError> {
        self.empty(
            Method::POST,
            &format!("/channels/{channel}/messages/{message}/ack"),
            &format!("POST /channels/{channel}/messages/:id/ack"),
            Some(&json!({ "token": null })),
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_query_carries_cursor() {
        let page = MessagePage {
            before: Some(Snowflake(77)),
            ..Default::default()
        };
        assert_eq!(page.query(), "limit=50&before=77");
    }

    #[test]
    fn limit_is_clamped_to_what_discord_accepts() {
        let page = MessagePage {
            limit: 250,
            ..Default::default()
        };
        assert_eq!(page.query(), "limit=100");
    }

    #[test]
    fn send_body_omits_unset_fields() {
        let body = serde_json::to_value(SendMessage {
            content: "hi".into(),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(body, json!({ "content": "hi" }));
    }

    #[test]
    fn reply_reference_serializes_ids_as_strings() {
        let body = serde_json::to_value(SendMessage {
            content: "yes".into(),
            nonce: Some("n1".into()),
            message_reference: Some(ReplyTarget {
                message_id: Snowflake(5),
                channel_id: Snowflake(6),
                guild_id: None,
            }),
        })
        .unwrap();
        assert_eq!(body["message_reference"]["message_id"], "5");
        assert_eq!(body["nonce"], "n1");
    }
}
