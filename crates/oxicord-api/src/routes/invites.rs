use oxicord_model::{InvitePreview, Snowflake};
use reqwest::Method;
use serde_json::json;

use crate::{ApiClient, ApiError};

impl ApiClient {
    /// Looks an invite up without using it.
    pub async fn resolve_invite(&self, code: &str) -> Result<InvitePreview, ApiError> {
        let response = self
            .send(
                Method::GET,
                &format!("/invites/{code}?with_counts=true&with_expiration=true"),
                "GET /invites/:code",
                None,
            )
            .await?;
        let bytes = response.bytes().await?;
        InvitePreview::from_json(code, &bytes)
            .ok_or_else(|| ApiError::Protocol("that invite isn't for a server".into()))
    }

    /// Joins the server an invite leads to. The server then arrives through
    /// the gateway like any other.
    pub async fn accept_invite(&self, code: &str, session_id: &str) -> Result<(), ApiError> {
        self.empty(
            Method::POST,
            &format!("/invites/{code}"),
            "POST /invites/:code",
            Some(&json!({ "session_id": session_id })),
        )
        .await
    }

    /// Makes a link into the server through `channel`. `max_age` is in
    /// seconds and 0 never expires, `max_uses` of 0 has no limit. Returns the
    /// invite code.
    pub async fn create_invite(&self, channel: Snowflake, max_age: u32, max_uses: u32) -> Result<String, ApiError> {
        let body = json!({ "max_age": max_age, "max_uses": max_uses, "temporary": false });
        let reply: serde_json::Value = self
            .json(
                Method::POST,
                &format!("/channels/{channel}/invites"),
                "POST /channels/:id/invites",
                Some(&body),
            )
            .await?;
        reply
            .get("code")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| ApiError::Protocol("the invite had no code".into()))
    }
}
