use oxicord_model::{Profile, Snowflake, User};
use reqwest::Method;

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
    ) -> Result<Profile, ApiError> {
        let mut path = format!("/users/{user}/profile?with_mutual_guilds=false");
        if let Some(guild) = guild {
            path.push_str(&format!("&guild_id={guild}"));
        }
        self.json(Method::GET, &path, "GET /users/:id/profile", None)
            .await
    }
}
