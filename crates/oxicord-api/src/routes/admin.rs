//! Server management: the calls behind the server settings pages.

use oxicord_model::{
    Snowflake,
    admin::{AuditLog, BanEntry, InviteEntry},
};
use reqwest::Method;
use serde_json::{Value, json};

use crate::{ApiClient, ApiError};

impl ApiClient {
    pub async fn guild_invites(&self, guild: Snowflake) -> Result<Vec<InviteEntry>, ApiError> {
        let body: Value = self
            .json(Method::GET, &format!("/guilds/{guild}/invites"), "GET /guilds/:id/invites", None)
            .await?;
        Ok(InviteEntry::parse_list(&body))
    }

    pub async fn revoke_invite(&self, code: &str) -> Result<(), ApiError> {
        self.empty(Method::DELETE, &format!("/invites/{code}"), "DELETE /invites/:code", None).await
    }

    pub async fn guild_bans(&self, guild: Snowflake) -> Result<Vec<BanEntry>, ApiError> {
        let body: Value = self
            .json(Method::GET, &format!("/guilds/{guild}/bans?limit=200"), "GET /guilds/:id/bans", None)
            .await?;
        Ok(BanEntry::parse_list(&body))
    }

    pub async fn unban(&self, guild: Snowflake, user: Snowflake) -> Result<(), ApiError> {
        self.empty(
            Method::DELETE,
            &format!("/guilds/{guild}/bans/{user}"),
            "DELETE /guilds/:id/bans/:id",
            None,
        )
        .await
    }

    pub async fn audit_log(&self, guild: Snowflake) -> Result<AuditLog, ApiError> {
        let body: Value = self
            .json(
                Method::GET,
                &format!("/guilds/{guild}/audit-logs?limit=50"),
                "GET /guilds/:id/audit-logs",
                None,
            )
            .await?;
        Ok(AuditLog::parse(&body))
    }

    /// Changes the server's own fields, such as `name` and `description`.
    pub async fn edit_guild(&self, guild: Snowflake, fields: &Value) -> Result<(), ApiError> {
        self.empty(Method::PATCH, &format!("/guilds/{guild}"), "PATCH /guilds/:id", Some(fields)).await
    }

    pub async fn create_role(&self, guild: Snowflake, name: &str) -> Result<(), ApiError> {
        self.empty(
            Method::POST,
            &format!("/guilds/{guild}/roles"),
            "POST /guilds/:id/roles",
            Some(&json!({ "name": name })),
        )
        .await
    }

    /// Changes a role: `name`, `color`, `hoist`, `mentionable`, `permissions`.
    pub async fn edit_role(&self, guild: Snowflake, role: Snowflake, fields: &Value) -> Result<(), ApiError> {
        self.empty(
            Method::PATCH,
            &format!("/guilds/{guild}/roles/{role}"),
            "PATCH /guilds/:id/roles/:id",
            Some(fields),
        )
        .await
    }

    pub async fn delete_role(&self, guild: Snowflake, role: Snowflake) -> Result<(), ApiError> {
        self.empty(
            Method::DELETE,
            &format!("/guilds/{guild}/roles/{role}"),
            "DELETE /guilds/:id/roles/:id",
            None,
        )
        .await
    }

    pub async fn rename_emoji(&self, guild: Snowflake, emoji: Snowflake, name: &str) -> Result<(), ApiError> {
        self.empty(
            Method::PATCH,
            &format!("/guilds/{guild}/emojis/{emoji}"),
            "PATCH /guilds/:id/emojis/:id",
            Some(&json!({ "name": name })),
        )
        .await
    }

    pub async fn delete_emoji(&self, guild: Snowflake, emoji: Snowflake) -> Result<(), ApiError> {
        self.empty(
            Method::DELETE,
            &format!("/guilds/{guild}/emojis/{emoji}"),
            "DELETE /guilds/:id/emojis/:id",
            None,
        )
        .await
    }

    /// `kind` is the channel type number, 0 for text and 2 for voice.
    pub async fn create_channel(
        &self,
        guild: Snowflake,
        name: &str,
        kind: u8,
        parent: Option<Snowflake>,
    ) -> Result<(), ApiError> {
        let mut body = json!({ "name": name, "type": kind });
        if let Some(parent) = parent {
            body["parent_id"] = json!(parent.to_string());
        }
        self.empty(
            Method::POST,
            &format!("/guilds/{guild}/channels"),
            "POST /guilds/:id/channels",
            Some(&body),
        )
        .await
    }

    /// Changes a channel: `name`, `topic`, `nsfw`, `rate_limit_per_user`.
    pub async fn edit_channel(&self, channel: Snowflake, fields: &Value) -> Result<(), ApiError> {
        self.empty(Method::PATCH, &format!("/channels/{channel}"), "PATCH /channels/:id", Some(fields)).await
    }
}
