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
}
