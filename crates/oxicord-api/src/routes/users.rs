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
}
