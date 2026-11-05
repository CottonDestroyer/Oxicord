//! What the server settings pages list: invites, bans and the audit log.
//! Parsing is forgiving, an entry we cannot read is dropped.

use serde_json::Value;

use crate::{Snowflake, User};

fn text(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_owned)
}

fn id(value: &Value, key: &str) -> Option<Snowflake> {
    value.get(key).and_then(Value::as_str).and_then(|s| s.parse().ok()).map(Snowflake)
}

fn count(value: &Value, key: &str) -> u64 {
    value.get(key).and_then(Value::as_u64).unwrap_or(0)
}

/// An invite link as the server lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InviteEntry {
    pub code: String,
    pub uses: u64,
    /// 0 means no limit.
    pub max_uses: u64,
    /// Seconds it lasts from creation, 0 for never.
    pub max_age: u64,
    pub created_at: Option<String>,
    pub inviter: Option<String>,
    pub channel: Option<String>,
}

impl InviteEntry {
    pub fn parse_list(body: &Value) -> Vec<Self> {
        body.as_array().map(|list| list.iter().filter_map(Self::parse).collect()).unwrap_or_default()
    }

    fn parse(value: &Value) -> Option<Self> {
        Some(Self {
            code: text(value, "code")?,
            uses: count(value, "uses"),
            max_uses: count(value, "max_uses"),
            max_age: count(value, "max_age"),
            created_at: text(value, "created_at"),
            inviter: value
                .get("inviter")
                .and_then(|u| text(u, "global_name").filter(|n| !n.is_empty()).or_else(|| text(u, "username"))),
            channel: value.get("channel").and_then(|c| text(c, "name")),
        })
    }
}

/// Someone banned from the server.
#[derive(Clone, Debug)]
pub struct BanEntry {
    pub user: User,
    pub reason: Option<String>,
}

impl BanEntry {
    pub fn parse_list(body: &Value) -> Vec<Self> {
        body.as_array().map(|list| list.iter().filter_map(Self::parse).collect()).unwrap_or_default()
    }

    fn parse(value: &Value) -> Option<Self> {
        Some(Self {
            user: serde_json::from_value(value.get("user")?.clone()).ok()?,
            reason: text(value, "reason").filter(|r| !r.is_empty()),
        })
    }
}

/// One line of the audit log.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuditEntry {
    pub id: Snowflake,
    pub user_id: Option<Snowflake>,
    pub target_id: Option<Snowflake>,
    pub action: u32,
    pub reason: Option<String>,
}

/// The audit log with the people it mentions, so lines can show names.
#[derive(Clone, Debug, Default)]
pub struct AuditLog {
    pub entries: Vec<AuditEntry>,
    pub users: Vec<User>,
}

impl AuditLog {
    pub fn parse(body: &Value) -> Self {
        let entries = body
            .get("audit_log_entries")
            .and_then(Value::as_array)
            .map(|list| {
                list.iter()
                    .filter_map(|e| {
                        Some(AuditEntry {
                            id: id(e, "id")?,
                            user_id: id(e, "user_id"),
                            target_id: id(e, "target_id"),
                            action: u32::try_from(e.get("action_type")?.as_u64()?).ok()?,
                            reason: text(e, "reason"),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        let users = body
            .get("users")
            .and_then(Value::as_array)
            .map(|list| list.iter().filter_map(|u| serde_json::from_value(u.clone()).ok()).collect())
            .unwrap_or_default();
        Self { entries, users }
    }
}

/// What an audit log action number means, in words.
pub fn action_name(action: u32) -> &'static str {
    match action {
        1 => "updated the server",
        10 => "created a channel",
        11 => "updated a channel",
        12 => "deleted a channel",
        13..=15 => "changed channel permissions",
        20 => "kicked a member",
        21 => "pruned members",
        22 => "banned a member",
        23 => "unbanned a member",
        24 => "updated a member",
        25 => "changed the roles of a member",
        26 => "moved a member",
        27 => "disconnected a member",
        28 => "added a bot",
        30 => "created a role",
        31 => "updated a role",
        32 => "deleted a role",
        40 => "created an invite",
        41 => "updated an invite",
        42 => "deleted an invite",
        50 => "created a webhook",
        51 => "updated a webhook",
        52 => "deleted a webhook",
        60 => "created an emoji",
        61 => "updated an emoji",
        62 => "deleted an emoji",
        72 => "deleted a message",
        73 => "bulk deleted messages",
        74 => "pinned a message",
        75 => "unpinned a message",
        80 => "created an integration",
        81 => "updated an integration",
        82 => "deleted an integration",
        90 => "created a stage",
        91 => "updated a stage",
        92 => "deleted a stage",
        100 => "created a sticker",
        101 => "updated a sticker",
        102 => "deleted a sticker",
        110 => "created a scheduled event",
        111 => "updated a scheduled event",
        112 => "deleted a scheduled event",
        120 => "created a thread",
        121 => "updated a thread",
        122 => "deleted a thread",
        140 => "created an auto moderation rule",
        141 => "updated an auto moderation rule",
        142 => "deleted an auto moderation rule",
        143..=145 => "triggered auto moderation",
        _ => "did something",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn invites_read_their_limits_and_makers() {
        let list = InviteEntry::parse_list(&json!([
            { "code": "abc", "uses": 3, "max_uses": 10, "max_age": 86400, "inviter": { "id": "1", "username": "ada" },
              "channel": { "name": "general" } },
            { "nonsense": true }
        ]));
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].inviter.as_deref(), Some("ada"));
        assert_eq!(list[0].channel.as_deref(), Some("general"));
        assert_eq!(list[0].max_age, 86400);
    }

    #[test]
    fn bans_carry_the_user_and_an_optional_reason() {
        let list = BanEntry::parse_list(&json!([
            { "user": { "id": "7", "username": "spam" }, "reason": "ads" },
            { "user": { "id": "8", "username": "other" }, "reason": null },
            { "reason": "no user" }
        ]));
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].reason.as_deref(), Some("ads"));
        assert!(list[1].reason.is_none());
    }

    #[test]
    fn the_audit_log_pairs_entries_with_users() {
        let log = AuditLog::parse(&json!({
            "audit_log_entries": [{ "id": "5", "user_id": "1", "target_id": "9", "action_type": 22, "reason": "spam" }],
            "users": [{ "id": "1", "username": "mod" }]
        }));
        assert_eq!(log.entries.len(), 1);
        assert_eq!(action_name(log.entries[0].action), "banned a member");
        assert_eq!(log.users[0].username, "mod");
    }
}
