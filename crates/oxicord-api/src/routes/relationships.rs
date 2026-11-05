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

    /// Sends a friend request by username. Discord may answer with a captcha
    /// challenge or a refusal, which comes back as the error.
    pub async fn add_friend_by_name(&self, username: &str) -> Result<(), ApiError> {
        self.empty(
            Method::POST,
            "/users/@me/relationships",
            "POST /users/@me/relationships",
            Some(&json!({ "username": username, "discriminator": null })),
        )
        .await
    }

    /// Blocks someone. Unblocking is removing the relationship.
    pub async fn block_user(&self, user: Snowflake) -> Result<(), ApiError> {
        self.empty(
            Method::PUT,
            &format!("/users/@me/relationships/{user}"),
            "PUT /users/@me/relationships/:id",
            Some(&json!({ "type": 2 })),
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
