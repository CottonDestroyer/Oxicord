use oxicord_model::Snowflake;
use reqwest::Method;
use serde_json::{Value, json};

use crate::{ApiClient, ApiError};

/// Discord takes at most this many read states per acknowledgement request.
pub const ACK_BATCH: usize = 100;

impl ApiClient {
    /// Changes our notification settings for a server. `settings` holds only
    /// the fields to change, and the new state comes back through the gateway.
    pub async fn patch_guild_settings(
        &self,
        guild: Snowflake,
        settings: &Value,
    ) -> Result<(), ApiError> {
        self.empty(
            Method::PATCH,
            &format!("/users/@me/guilds/{guild}/settings"),
            "PATCH /users/@me/guilds/:id/settings",
            Some(settings),
        )
        .await
    }

    /// Marks many channels read in one go, each up to the given message.
    pub async fn ack_bulk(&self, channels: &[(Snowflake, Snowflake)]) -> Result<(), ApiError> {
        for batch in channels.chunks(ACK_BATCH) {
            let read_states: Vec<Value> = batch
                .iter()
                .map(|(channel, message)| {
                    json!({
                        "channel_id": channel.to_string(),
                        "message_id": message.to_string(),
                        // 0 is an ordinary channel's read state.
                        "read_state_type": 0,
                    })
                })
                .collect();
            self.empty(
                Method::POST,
                "/read-states/ack-bulk",
                "POST /read-states/ack-bulk",
                Some(&json!({ "read_states": read_states })),
            )
            .await?;
        }
        Ok(())
    }

    /// Our own nickname in `guild`, if we set one.
    pub async fn own_nickname(&self, guild: Snowflake) -> Result<Option<String>, ApiError> {
        let member: oxicord_model::Member = self
            .json(
                Method::GET,
                &format!("/users/@me/guilds/{guild}/member"),
                "GET /users/@me/guilds/:id/member",
                None,
            )
            .await?;
        Ok(member.nick.filter(|n| !n.is_empty()))
    }

    /// Sets our nickname in `guild`, or clears it with `None`.
    pub async fn set_nickname(&self, guild: Snowflake, nick: Option<&str>) -> Result<(), ApiError> {
        self.empty(
            Method::PATCH,
            &format!("/guilds/{guild}/members/@me"),
            "PATCH /guilds/:id/members/@me",
            Some(&json!({ "nick": nick })),
        )
        .await
    }

    pub async fn leave_guild(&self, guild: Snowflake) -> Result<(), ApiError> {
        self.empty(
            Method::DELETE,
            &format!("/users/@me/guilds/{guild}"),
            "DELETE /users/@me/guilds/:id",
            Some(&json!({ "lurking": false })),
        )
        .await
    }

    /// Creates a new server with the given name. The new guild arrives through
    /// the gateway as `GUILD_CREATE`, which the store already ingests, so the
    /// server shows up at once.
    pub async fn create_guild(&self, name: &str) -> Result<oxicord_model::Guild, ApiError> {
        let guild: oxicord_model::Guild = self
            .json(
                Method::POST,
                "/guilds",
                "POST /guilds",
                Some(&json!({ "name": name })),
            )
            .await?;
        Ok(guild)
    }
}
