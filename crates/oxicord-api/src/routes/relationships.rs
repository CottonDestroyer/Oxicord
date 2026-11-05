use oxicord_model::{Channel, Snowflake};
use reqwest::Method;
use serde_json::json;

use crate::{ApiClient, ApiError};

impl ApiClient {
    /// Opens (or returns the existing) one-to-one conversation with `user`.
    pub async fn open_dm(&self, user: Snowflake) -> Result<Channel, ApiError> {
        let body = json!({ "recipients": [user.to_string()] });
        self.json(
            Method::POST,
            "/users/@me/channels",
            "POST /users/@me/channels",
            Some(&body),
        )
        .await
    }

    /// Accepts an incoming request. The same call sends one, but the friends
    /// page only uses it to accept.
    pub async fn accept_friend(&self, user: Snowflake) -> Result<(), ApiError> {
        self.empty(
            Method::PUT,
            &format!("/users/@me/relationships/{user}"),
            "PUT /users/@me/relationships/:id",
            Some(&json!({})),
        )
        .await
    }

    /// Removes a friend, declines a request or cancels one we sent. Discord
    /// uses one endpoint for all three.
    pub async fn remove_relationship(&self, user: Snowflake) -> Result<(), ApiError> {
        self.empty(
            Method::DELETE,
            &format!("/users/@me/relationships/{user}"),
            "DELETE /users/@me/relationships/:id",
            None,
        )
        .await
    }
}
