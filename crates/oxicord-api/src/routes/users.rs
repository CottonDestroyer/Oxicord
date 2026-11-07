use oxicord_model::{Snowflake, User};
use reqwest::Method;
use serde::Deserialize;

use crate::{ApiClient, ApiError};

impl ApiClient {
    /// Also serves as the cheapest way to check that a token still works.
    pub async fn current_user(&self) -> Result<User, ApiError> {
        self.json(Method::GET, "/users/@me", "GET /users/@me", None)
            .await
    }

    /// Another person's public details, for names we haven't met yet.
    pub async fn user(&self, id: Snowflake) -> Result<User, ApiError> {
        self.json(Method::GET, &format!("/users/{id}"), "GET /users/:id", None).await
    }

    /// The details a profile card shows. `guild` picks up the per-server
    /// bio when the person has set one.
    pub async fn profile(
        &self,
        user: Snowflake,
        guild: Option<Snowflake>,
    ) -> Result<oxicord_model::Profile, ApiError> {
        let mut path = format!("/users/{user}/profile?with_mutual_guilds=true&with_mutual_friends_count=true");
        if let Some(guild) = guild {
            path.push_str(&format!("&guild_id={guild}"));
        }
        self.json(Method::GET, &path, "GET /users/:id/profile", None)
            .await
    }

    /// The note we wrote about a user, visible only to us.
    pub async fn user_note(&self, user: Snowflake) -> Result<String, ApiError> {
        #[derive(Deserialize)]
        struct NoteResponse {
            #[serde(default)]
            note: String,
        }
        let response: NoteResponse = self
            .json(
                Method::GET,
                &format!("/users/@me/notes/{user}"),
                "GET /users/@me/notes/:id",
                None,
            )
            .await?;
        Ok(response.note)
    }

    /// Save a message for later. Discord records it under saved messages.
    pub async fn bookmark_message(
        &self,
        channel: Snowflake,
        message: Snowflake,
    ) -> Result<(), ApiError> {
        self.empty(
            Method::PUT,
            &format!("/users/@me/saved-messages/{channel}/{message}"),
            "PUT /users/@me/saved-messages/:channel/:message",
            Some(&serde_json::json!({"source":"context_menu"})),
        )
        .await
    }
}