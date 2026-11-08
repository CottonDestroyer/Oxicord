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
    /// 1 makes the new message a forward of the referenced one.
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub kind: Option<u8>,
}

/// A file to attach, already read into memory.
#[derive(Clone, Debug)]
pub struct Upload {
    pub filename: String,
    pub data: std::sync::Arc<Vec<u8>>,
    /// Alt text for the attachment, shown to screen readers.
    pub description: Option<String>,
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
    /// Stickers to send, which need no text.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub sticker_ids: Vec<String>,
    /// Only set to silence the ping a reply sends to its author.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_mentions: Option<AllowedMentions>,
    /// A poll attached to this message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub poll: Option<PollCreate>,
}

/// A poll to attach to a new message.
#[derive(Clone, Debug, Serialize)]
pub struct PollCreate {
    pub question: PollMedia,
    pub answers: Vec<PollAnswerCreate>,
    /// Hours until the poll closes.
    pub duration: u32,
    #[serde(rename = "allow_multiselect")]
    pub multiselect: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct PollMedia {
    pub text: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct PollAnswerCreate {
    pub poll_media: PollMedia,
}

/// Which mentions in a message may notify. Naming every kind but leaving
/// `replied_user` false is how the official client mutes a reply ping.
#[derive(Clone, Debug, Serialize)]
pub struct AllowedMentions {
    pub parse: Vec<&'static str>,
    pub replied_user: bool,
}

impl AllowedMentions {
    pub fn without_reply_ping() -> Self {
        Self { parse: vec!["users", "roles", "everyone"], replied_user: false }
    }
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

    /// Sends a message with files attached. The text and reply go in a JSON
    /// part, the way the official client does it, with each file named by
    /// index so Discord can match the two.
    pub async fn send_message_with_files(
        &self,
        channel: Snowflake,
        message: &SendMessage,
        files: &[Upload],
    ) -> Result<Message, ApiError> {
        let mut payload = serde_json::to_value(message)?;
        payload["attachments"] = files
            .iter()
            .enumerate()
            .map(|(id, f)| {
                let mut entry = json!({ "id": id, "filename": f.filename });
                if let Some(desc) = &f.description
                    && !desc.trim().is_empty()
                {
                    entry["description"] = json!(desc);
                }
                entry
            })
            .collect();
        let payload = payload.to_string();

        let path = format!("/channels/{channel}/messages");
        let route = format!("POST /channels/{channel}/messages");
        let resp = self
            .send_with(Method::POST, &path, &route, |req| {
                let mut form = reqwest::multipart::Form::new().part(
                    "payload_json",
                    reqwest::multipart::Part::text(payload.clone())
                        .mime_str("application/json")
                        .expect("a fixed mime type is valid"),
                );
                for (index, file) in files.iter().enumerate() {
                    form = form.part(
                        format!("files[{index}]"),
                        reqwest::multipart::Part::bytes(file.data.to_vec())
                            .file_name(file.filename.clone()),
                    );
                }
                req.multipart(form)
            })
            .await?;
        let bytes = resp.bytes().await?;
        Ok(serde_json::from_slice(&bytes)?)
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
    /// Moves the read marker back to `message` by hand, which is how Mark
    /// Unread works: the server keeps what follows as unread.
    pub async fn mark_unread(&self, channel: Snowflake, message: Snowflake) -> Result<(), ApiError> {
        self.empty(
            Method::POST,
            &format!("/channels/{channel}/messages/{message}/ack"),
            &format!("POST /channels/{channel}/messages/:id/ack"),
            Some(&json!({ "manual": true, "mention_count": 0 })),
        )
        .await
    }

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

    /// Publish an announcement message to every channel following this one.
    pub async fn crosspost_message(
        &self,
        channel: Snowflake,
        message: Snowflake,
    ) -> Result<Message, ApiError> {
        self.json(
            Method::POST,
            &format!("/channels/{channel}/messages/{message}/crosspost"),
            &format!("POST /channels/{channel}/messages/{message}/crosspost"),
            None,
        )
        .await
    }

    /// End a poll early, freezing its results.
    pub async fn expire_poll(&self, channel: Snowflake, message: Snowflake) -> Result<(), ApiError> {
        self.empty(
            Method::POST,
            &format!("/channels/{channel}/polls/{message}/expire"),
            &format!("POST /channels/{channel}/polls/{message}/expire"),
            None,
        )
        .await
    }

    /// The people who voted for `answer` on a poll.
    pub async fn poll_voters(&self, channel: Snowflake, message: Snowflake, answer: u32) -> Result<Vec<oxicord_model::User>, ApiError> {
        #[derive(serde::Deserialize)]
        struct Body { #[serde(default)] users: Vec<oxicord_model::User> }
        let resp: Body = self
            .json(
                Method::GET,
                &format!("/channels/{channel}/polls/{message}/answers/{answer}"),
                &format!("GET /channels/{channel}/polls/{message}/answers/:id"),
                None,
            )
            .await?;
        Ok(resp.users)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_silenced_reply_names_every_mention_kind_but_the_replied_user() {
        let quiet = SendMessage { allowed_mentions: Some(AllowedMentions::without_reply_ping()), ..Default::default() };
        let body = serde_json::to_value(&quiet).unwrap();
        assert_eq!(body["allowed_mentions"]["replied_user"], false);
        assert_eq!(body["allowed_mentions"]["parse"], serde_json::json!(["users", "roles", "everyone"]));
        // Left alone the field is not sent at all.
        assert!(serde_json::to_value(SendMessage::default()).unwrap().get("allowed_mentions").is_none());
    }

    #[test]
    fn a_forward_is_a_reference_of_type_one() {
        let forward = SendMessage {
            message_reference: Some(ReplyTarget { message_id: Snowflake(1), channel_id: Snowflake(2), guild_id: None, kind: Some(1) }),
            ..Default::default()
        };
        let body = serde_json::to_value(&forward).unwrap();
        assert_eq!(body["message_reference"]["type"], 1);
        assert_eq!(body["message_reference"]["message_id"], "1");
        let reply = ReplyTarget { message_id: Snowflake(1), channel_id: Snowflake(2), guild_id: None, kind: None };
        assert!(serde_json::to_value(reply).unwrap().get("type").is_none());
    }

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
                kind: None,
            }),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(body["message_reference"]["message_id"], "5");
        assert_eq!(body["nonce"], "n1");
    }
}
