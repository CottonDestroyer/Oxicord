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
        let mut path = format!(
            "/users/{user}/profile?with_mutual_guilds=true&with_mutual_friends=true&with_mutual_friends_count=true"
        );
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

    /// Overwrite the private note we keep about a user. The note is ours
    /// alone and shows nowhere but our own client.
    pub async fn set_user_note(&self, user: Snowflake, note: &str) -> Result<(), ApiError> {
        self.empty(
            Method::PATCH,
            &format!("/users/@me/notes/{user}"),
            "PATCH /users/@me/notes/:id",
            Some(&serde_json::json!({ "note": note })),
        )
        .await
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

    /// What a profile effect looks like, from the shop catalog. Nothing for
    /// a listing that is not an effect.
    pub async fn profile_effect(
        &self,
        sku: Snowflake,
        pick: usize,
    ) -> Result<Option<oxicord_model::ProfileEffect>, ApiError> {
        let product: serde_json::Value = self
            .json(
                Method::GET,
                &format!("/collectibles-products/{sku}"),
                "GET /collectibles-products/:sku",
                None,
            )
            .await?;
        Ok(oxicord_model::ProfileEffect::from_product(product, sku, pick))
    }

    /// What a profile frame looks like, from the shop catalog. Nothing for
    /// a listing that is not a frame.
    pub async fn profile_frame(
        &self,
        sku: Snowflake,
    ) -> Result<Option<oxicord_model::ProfileFrame>, ApiError> {
        let product: serde_json::Value = self
            .json(
                Method::GET,
                &format!("/collectibles-products/{sku}"),
                "GET /collectibles-products/:sku",
                None,
            )
            .await?;
        Ok(oxicord_model::ProfileFrame::from_product(product, sku))
    }

    /// Names and covers of games, for the profile's game widgets. Games
    /// Discord doesn't know are left out.
    pub async fn games(
        &self,
        ids: &[Snowflake],
    ) -> Result<std::collections::HashMap<Snowflake, oxicord_model::Game>, ApiError> {
        #[derive(Deserialize)]
        struct Media {
            #[serde(default)]
            cover_url: Option<String>,
            #[serde(default)]
            cover: Option<Cover>,
        }
        #[derive(Deserialize)]
        struct Cover {
            #[serde(rename = "type", default)]
            kind: String,
            #[serde(default)]
            value: String,
        }
        #[derive(Deserialize)]
        struct Game {
            id: Snowflake,
            #[serde(default)]
            name: String,
            #[serde(default)]
            icon_hash: Option<String>,
            #[serde(default)]
            media: Option<Media>,
        }
        if ids.is_empty() {
            return Ok(Default::default());
        }
        let query: Vec<String> = ids.iter().map(|id| format!("game_ids={id}")).collect();
        let games: Vec<Game> = self
            .json(Method::GET, &format!("/games?{}", query.join("&")), "GET /games", None)
            .await?;
        Ok(games
            .into_iter()
            .map(|game| {
                let id = game.id;
                // The cover is either a picture hosted elsewhere or a hash of
                // one of the application's own assets.
                let cover_url = game.media.and_then(|media| {
                    media.cover_url.filter(|u| !u.is_empty()).or_else(|| {
                        media.cover.filter(|c| !c.value.is_empty()).map(|c| {
                            if c.kind == "url" {
                                c.value
                            } else {
                                format!("https://cdn.discordapp.com/app-icons/{id}/{}.png?size=256", c.value)
                            }
                        })
                    })
                });
                let cover_url = cover_url.or_else(|| {
                    game.icon_hash
                        .filter(|h| !h.is_empty())
                        .map(|h| format!("https://cdn.discordapp.com/app-icons/{id}/{h}.png?size=256"))
                });
                (id, oxicord_model::Game { name: game.name, cover_url })
            })
            .collect())
    }
}
