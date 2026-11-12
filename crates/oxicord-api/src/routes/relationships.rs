use oxicord_model::{Channel, Snowflake};
use reqwest::Method;
use serde_json::json;

use crate::{ApiClient, ApiError};

impl ApiClient {
    /// Opens (or returns the existing) one-to-one conversation with `user`.
    pub async fn open_dm(&self, user: Snowflake) -> Result<Channel, ApiError> {
        self.open_conversation(&[user]).await
    }

    /// Starts a group conversation, or a one-to-one one for a single person.
    pub async fn open_conversation(&self, users: &[Snowflake]) -> Result<Channel, ApiError> {
        let ids: Vec<String> = users.iter().map(ToString::to_string).collect();
        let body = json!({ "recipients": ids });
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

    /// Sends a friend request by username, with the optional note that
    /// shows in the DM once they accept. Discord may answer with a captcha
    /// challenge or a refusal, which comes back as the error.
    pub async fn add_friend_by_name(&self, username: &str, note: Option<&str>) -> Result<(), ApiError> {
        self.empty(
            Method::POST,
            "/users/@me/relationships",
            "POST /users/@me/relationships",
            Some(&friend_request_body(username, note)),
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

    /// Sets the personal nickname we keep for a friend, or clears it with
    /// `None`. Discord stores it on the relationship and shows it in DMs.
    pub async fn set_relationship_nick(&self, user: Snowflake, nick: Option<&str>) -> Result<(), ApiError> {
        self.empty(
            Method::PATCH,
            &format!("/users/@me/relationships/{user}"),
            "PATCH /users/@me/relationships/:id",
            Some(&json!({ "nickname": nick })),
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

    /// Hides a conversation from the list, or leaves a group. Messages stay.
    pub async fn close_channel(&self, channel: Snowflake) -> Result<(), ApiError> {
        self.empty(Method::DELETE, &format!("/channels/{channel}"), "DELETE /channels/:id", None).await
    }

    /// Adds a friend to an existing group conversation.
    pub async fn add_recipient(&self, channel: Snowflake, user: Snowflake) -> Result<(), ApiError> {
        self.empty(
            Method::PUT,
            &format!("/channels/{channel}/recipients/{user}"),
            "PUT /channels/:id/recipients/:id",
            None,
        )
        .await
    }
}

/// The body of a friend request. The note is left out when it is empty, the
/// way the web client sends a request without one.
fn friend_request_body(username: &str, note: Option<&str>) -> serde_json::Value {
    let mut body = json!({ "username": username, "discriminator": null });
    if let Some(note) = note.map(str::trim).filter(|n| !n.is_empty()) {
        body["note"] = json!(note);
    }
    body
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_request_carries_the_note_only_when_there_is_one() {
        assert_eq!(
            friend_request_body("wumpus", None),
            json!({ "username": "wumpus", "discriminator": null })
        );
        assert_eq!(friend_request_body("wumpus", Some("  ")), friend_request_body("wumpus", None));
        assert_eq!(
            friend_request_body("wumpus", Some(" hi, it's me ")),
            json!({ "username": "wumpus", "discriminator": null, "note": "hi, it's me" })
        );
    }
}
