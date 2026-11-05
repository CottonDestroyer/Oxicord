use oxicord_model::{Channel, Message, Snowflake, ThreadPage};
use reqwest::Method;
use serde::Deserialize;
use serde_json::json;

use crate::{ApiClient, ApiError};

/// Threads shown per request, matching the official client.
pub const THREAD_PAGE: u32 = 25;

#[derive(Deserialize)]
struct RawPage {
    #[serde(default)]
    threads: Vec<Channel>,
    #[serde(default)]
    members: Vec<RawMember>,
    #[serde(default)]
    has_more: bool,
    #[serde(default)]
    first_messages: Vec<Message>,
}

#[derive(Deserialize)]
struct RawMember {
    id: Snowflake,
}

impl ApiClient {
    /// The threads of a channel, open ones first when `archived` is false.
    pub async fn threads(
        &self,
        parent: Snowflake,
        archived: bool,
        offset: u32,
    ) -> Result<ThreadPage, ApiError> {
        let path = format!(
            "/channels/{parent}/threads/search?archived={archived}&sort_by=last_message_time\
             &sort_order=desc&limit={THREAD_PAGE}&offset={offset}"
        );
        let raw: RawPage = self
            .json(
                Method::GET,
                &path,
                &format!("GET /channels/{parent}/threads/search"),
                None,
            )
            .await?;
        Ok(ThreadPage {
            threads: raw.threads,
            joined: raw.members.into_iter().map(|m| m.id).collect(),
            has_more: raw.has_more,
            first_messages: raw.first_messages,
        })
    }

    pub async fn join_thread(&self, thread: Snowflake) -> Result<(), ApiError> {
        self.empty(
            Method::POST,
            &format!("/channels/{thread}/thread-members/@me?location=Banner"),
            "POST /channels/:id/thread-members/@me",
            None,
        )
        .await
    }

    pub async fn leave_thread(&self, thread: Snowflake) -> Result<(), ApiError> {
        self.empty(
            Method::DELETE,
            &format!("/channels/{thread}/thread-members/@me?location=Sidebar%20Overflow"),
            "DELETE /channels/:id/thread-members/@me",
            None,
        )
        .await
    }

    /// Starts a thread on a message. The duration is in minutes.
    pub async fn create_thread(
        &self,
        channel: Snowflake,
        message: Snowflake,
        name: &str,
    ) -> Result<Channel, ApiError> {
        let body = json!({ "name": name, "auto_archive_duration": 1440 });
        self.json(
            Method::POST,
            &format!("/channels/{channel}/messages/{message}/threads"),
            &format!("POST /channels/{channel}/messages/:id/threads"),
            Some(&body),
        )
        .await
    }
}

/// What a forum post list needs about each post: its opening message.
#[derive(Deserialize)]
struct PostData {
    #[serde(default)]
    threads: std::collections::HashMap<String, PostEntry>,
}

#[derive(Deserialize)]
struct PostEntry {
    #[serde(default)]
    first_message: Option<Message>,
}

impl ApiClient {
    /// The opening messages of some posts of a forum, which the list shows as
    /// previews. Each returned message belongs to its post's channel.
    pub async fn forum_previews(
        &self,
        forum: Snowflake,
        posts: &[Snowflake],
    ) -> Result<Vec<Message>, ApiError> {
        let body = json!({ "thread_ids": posts.iter().map(Snowflake::to_string).collect::<Vec<_>>() });
        let data: PostData = self
            .json(
                Method::POST,
                &format!("/channels/{forum}/post-data"),
                &format!("POST /channels/{forum}/post-data"),
                Some(&body),
            )
            .await?;
        Ok(data.threads.into_values().filter_map(|e| e.first_message).collect())
    }

    /// Starts a forum post: a thread together with its opening message.
    pub async fn create_forum_post(
        &self,
        forum: Snowflake,
        title: &str,
        content: &str,
        tags: &[Snowflake],
    ) -> Result<Channel, ApiError> {
        let body = json!({
            "name": title,
            "auto_archive_duration": 4320,
            "applied_tags": tags.iter().map(Snowflake::to_string).collect::<Vec<_>>(),
            "message": { "content": content },
        });
        self.json(
            Method::POST,
            &format!("/channels/{forum}/threads?use_nested_fields=true"),
            &format!("POST /channels/{forum}/threads"),
            Some(&body),
        )
        .await
    }
}
