use oxicord_model::{Message, Snowflake, User};
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

    /// Remove a saved/bookmarked message.
    pub async fn unbookmark_message(
        &self,
        channel: Snowflake,
        message: Snowflake,
    ) -> Result<(), ApiError> {
        self.empty(
            Method::DELETE,
            &format!("/users/@me/saved-messages/{channel}/{message}"),
            "DELETE /users/@me/saved-messages/:channel/:message",
            None,
        )
        .await
    }

    /// Our saved/bookmarked messages, including reminder due times.
    pub async fn saved_messages(&self) -> Result<Vec<oxicord_model::SavedMessage>, ApiError> {
        #[derive(Deserialize)]
        struct SavedMessages {
            #[serde(default)]
            results: Vec<oxicord_model::SavedMessage>,
        }
        let response: SavedMessages = self
            .json(
                Method::GET,
                "/users/@me/saved-messages/bookmarks?limit=25",
                "GET /users/@me/saved-messages/bookmarks",
                None,
            )
            .await?;
        Ok(response.results)
    }

    /// Messages scheduled to send later.
    pub async fn scheduled_messages(&self) -> Result<Vec<Message>, ApiError> {
        self.json(
            Method::GET,
            "/users/@me/scheduled-messages",
            "GET /users/@me/scheduled-messages",
            None,
        )
        .await
    }

    /// The display name of a game, for a profile's favorite game.
    pub async fn game_name(&self, game: Snowflake) -> Result<String, ApiError> {
        #[derive(Deserialize)]
        struct Game {
            name: String,
        }
        let games: Vec<Game> = self
            .json(
                Method::GET,
                &format!("/games?game_ids={game}"),
                "GET /games",
                None,
            )
            .await?;
        Ok(games.into_iter().next().map(|game| game.name).unwrap_or_default())
    }
}