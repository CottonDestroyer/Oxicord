use oxicord_model::Snowflake;
use reqwest::Method;

use crate::{ApiClient, ApiError};

impl ApiClient {
    /// Shows the "is typing" indicator for roughly ten seconds.
    pub async fn trigger_typing(&self, channel: Snowflake) -> Result<(), ApiError> {
        self.empty(
            Method::POST,
            &format!("/channels/{channel}/typing"),
            &format!("POST /channels/{channel}/typing"),
            None,
        )
        .await
    }

    /// Make an announcement channel's messages arrive in `target`.
    pub async fn follow_channel(
        &self,
        source: Snowflake,
        target: Snowflake,
    ) -> Result<(), ApiError> {
        self.empty(
            Method::POST,
            &format!("/channels/{source}/followers"),
            &format!("POST /channels/{source}/followers"),
            Some(&serde_json::json!({ "webhook_channel_id": target.to_string() })),
        )
        .await
    }
}
