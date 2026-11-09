use oxicord_model::Snowflake;
use reqwest::Method;

use crate::{ApiClient, ApiError};

impl ApiClient {
    /// The consent status that accepts a message request. The other
    /// statuses (0 unspecified, 1 pending, 3 rejected) are not ours to set.
    pub const CONSENT_ACCEPTED: u64 = 2;

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

    /// Accepts a message request. The answer arrives as a CHANNEL_UPDATE
    /// that clears the request flag.
    pub async fn accept_message_request(&self, channel: Snowflake) -> Result<(), ApiError> {
        self.empty(
            Method::PUT,
            &format!("/channels/{channel}/recipients/@me"),
            &format!("PUT /channels/{channel}/recipients/@me"),
            Some(&serde_json::json!({ "consent_status": Self::CONSENT_ACCEPTED })),
        )
        .await
    }

    /// Ignores a message request. The server closes the conversation and
    /// answers with a CHANNEL_DELETE.
    pub async fn ignore_message_request(&self, channel: Snowflake) -> Result<(), ApiError> {
        self.empty(
            Method::DELETE,
            &format!("/channels/{channel}/recipients/@me"),
            &format!("DELETE /channels/{channel}/recipients/@me"),
            None,
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_accepted_request_sends_consent_status_two() {
        let body = serde_json::json!({ "consent_status": ApiClient::CONSENT_ACCEPTED });
        assert_eq!(body["consent_status"], serde_json::json!(2));
    }
}
