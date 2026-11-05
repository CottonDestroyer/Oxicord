use oxicord_model::InvitePreview;
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
}
