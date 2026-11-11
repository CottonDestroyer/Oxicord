//! Server management: the calls behind the server settings pages.

use oxicord_model::{
    Permissions, Snowflake,
    admin::{AuditLog, BanEntry, InviteEntry, WebhookEntry},
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

    /// Removes a member from the server. Requires the `Kick Members` permission
    /// and a role high enough to target the member.
    pub async fn kick_member(&self, guild: Snowflake, user: Snowflake) -> Result<(), ApiError> {
        self.empty(
            Method::DELETE,
            &format!("/guilds/{guild}/members/{user}"),
            "DELETE /guilds/:id/members/:id",
            None,
        )
        .await
    }

    /// Bans a member, optionally wiping their recent messages. Requires the
    /// `Ban Members` permission and a role high enough to target them.
    pub async fn ban_member(
        &self,
        guild: Snowflake,
        user: Snowflake,
        delete_message_days: u32,
    ) -> Result<(), ApiError> {
        self.empty(
            Method::PUT,
            &format!("/guilds/{guild}/bans/{user}"),
            "PUT /guilds/:id/bans/:id",
            Some(&json!({ "delete_message_seconds": delete_message_days * 86_400 })),
        )
        .await
    }

    /// Times a member out until `until` (an RFC3339 string), or lifts an
    /// existing timeout with `None`. Requires the `Timeout Members` permission
    /// and a role high enough to target them.
    pub async fn set_timeout(
        &self,
        guild: Snowflake,
        user: Snowflake,
        until: Option<String>,
    ) -> Result<(), ApiError> {
        self.empty(
            Method::PATCH,
            &format!("/guilds/{guild}/members/{user}"),
            "PATCH /guilds/:id/members/:id",
            Some(&json!({ "communication_disabled_until": until })),
        )
        .await
    }

    /// Replaces a member's roles. Requires the `Manage Roles` permission and a
    /// role hierarchy high enough to target them. `@everyone` (the role whose
    /// id equals the guild) is always included and cannot be removed.
    pub async fn set_member_roles(
        &self,
        guild: Snowflake,
        user: Snowflake,
        roles: Vec<Snowflake>,
    ) -> Result<(), ApiError> {
        self.empty(
            Method::PATCH,
            &format!("/guilds/{guild}/members/{user}"),
            "PATCH /guilds/:id/members/:id",
            Some(&json!({ "roles": roles })),
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

    /// Sets what a role or member may do in one channel. `kind` is 0 for a role
    /// and 1 for a member; the two permission sets are sent as strings, which is
    /// what the web client sends (module `updatePermissionOverwrite`). Gated by
    /// the admin-write rule: built and unit tested, never sent against a real
    /// server.
    pub async fn set_overwrite(
        &self,
        channel: Snowflake,
        target: Snowflake,
        kind: u8,
        allow: Permissions,
        deny: Permissions,
    ) -> Result<(), ApiError> {
        self.empty(
            Method::PUT,
            &format!("/channels/{channel}/permissions/{target}"),
            "PUT /channels/:id/permissions/:id",
            Some(&overwrite_body(target, kind, allow, deny)),
        )
        .await
    }

    /// Removes a role's or member's overwrite from a channel, so it inherits.
    pub async fn clear_overwrite(&self, channel: Snowflake, target: Snowflake) -> Result<(), ApiError> {
        self.empty(
            Method::DELETE,
            &format!("/channels/{channel}/permissions/{target}"),
            "DELETE /channels/:id/permissions/:id",
            None,
        )
        .await
    }

    /// Changes a channel: `name`, `topic`, `nsfw`, `rate_limit_per_user`.
    pub async fn edit_channel(&self, channel: Snowflake, fields: &Value) -> Result<(), ApiError> {
        self.empty(Method::PATCH, &format!("/channels/{channel}"), "PATCH /channels/:id", Some(fields)).await
    }

    /// Every webhook in the server. Needs `Manage Webhooks`.
    pub async fn guild_webhooks(&self, guild: Snowflake) -> Result<Vec<WebhookEntry>, ApiError> {
        let body: Value = self
            .json(Method::GET, &format!("/guilds/{guild}/webhooks"), "GET /guilds/:id/webhooks", None)
            .await?;
        Ok(WebhookEntry::parse_list(&body))
    }

    /// Makes a webhook in a channel. The web client sends only the name.
    pub async fn create_webhook(&self, channel: Snowflake, name: &str) -> Result<(), ApiError> {
        self.empty(
            Method::POST,
            &format!("/channels/{channel}/webhooks"),
            "POST /channels/:id/webhooks",
            Some(&json!({ "name": name })),
        )
        .await
    }

    /// Changes a webhook's name and the channel it posts to.
    pub async fn edit_webhook(&self, webhook: Snowflake, name: &str, channel: Snowflake) -> Result<(), ApiError> {
        self.empty(
            Method::PATCH,
            &format!("/webhooks/{webhook}"),
            "PATCH /webhooks/:id",
            Some(&webhook_body(name, channel)),
        )
        .await
    }

    pub async fn delete_webhook(&self, webhook: Snowflake) -> Result<(), ApiError> {
        self.empty(Method::DELETE, &format!("/webhooks/{webhook}"), "DELETE /webhooks/:id", None).await
    }
}

/// The edit of one webhook, its channel as a string id.
fn webhook_body(name: &str, channel: Snowflake) -> Value {
    json!({ "name": name, "channel_id": channel.to_string() })
}

/// The body of one overwrite: its target, whether that is a role (0) or a
/// member (1), and what it allows and denies, the sets as strings.
fn overwrite_body(target: Snowflake, kind: u8, allow: Permissions, deny: Permissions) -> Value {
    json!({
        "id": target.to_string(),
        "type": kind,
        "allow": allow.bits().to_string(),
        "deny": deny.bits().to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_webhook_edit_names_the_webhook_and_its_channel_as_a_string() {
        assert_eq!(webhook_body("Hook", Snowflake(42)), json!({ "name": "Hook", "channel_id": "42" }));
    }

    #[test]
    fn an_overwrite_names_its_target_kind_and_both_sets_as_strings() {
        let body = overwrite_body(
            Snowflake(77),
            0,
            Permissions::SEND_MESSAGES,
            Permissions::VIEW_CHANNEL | Permissions::PIN_MESSAGES,
        );
        assert_eq!(
            body,
            json!({
                "id": "77",
                "type": 0,
                "allow": "2048",
                "deny": (1u64 << 51 | 1 << 10).to_string(),
            })
        );
        // A member is type 1 and an empty set is still sent.
        let member = overwrite_body(Snowflake(5), 1, Permissions::empty(), Permissions::empty());
        assert_eq!(member["type"], 1);
        assert_eq!(member["allow"], "0");
    }
}
