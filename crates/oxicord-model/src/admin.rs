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

/// A webhook as the server lists it. Followed channels (type 2) and
/// application webhooks (type 3) carry no token, so they have no link to copy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebhookEntry {
    pub id: Snowflake,
    pub name: String,
    pub channel: Option<Snowflake>,
    pub avatar: Option<String>,
    pub token: Option<String>,
    /// 1 incoming, 2 channel follower, 3 application.
    pub kind: u8,
    pub creator: Option<String>,
}

impl WebhookEntry {
    pub fn parse_list(body: &Value) -> Vec<Self> {
        body.as_array().map(|list| list.iter().filter_map(Self::parse).collect()).unwrap_or_default()
    }

    pub fn parse(value: &Value) -> Option<Self> {
        Some(Self {
            id: id(value, "id")?,
            name: text(value, "name").unwrap_or_default(),
            channel: id(value, "channel_id"),
            avatar: text(value, "avatar"),
            token: text(value, "token"),
            kind: u8::try_from(count(value, "type")).unwrap_or(1),
            creator: value.get("user").and_then(|u| {
                // The web client shows the old tag, `name#1234`, when there is one.
                let name = text(u, "username")?;
                match text(u, "discriminator").filter(|d| d != "0" && !d.is_empty()) {
                    Some(tag) => Some(format!("{name}#{tag}")),
                    None => Some(name),
                }
            }),
        })
    }

    /// The address that posts into the channel, when there is one.
    pub fn url(&self) -> Option<String> {
        self.token.as_ref().map(|token| format!("https://discord.com/api/webhooks/{}/{token}", self.id))
    }
}

/// One AutoMod rule as the server lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AutoModRule {
    pub id: Snowflake,
    pub name: String,
    /// 1 keyword, 3 spam, 4 keyword preset, 5 mention spam, 6 member profile.
    pub trigger: u8,
    pub enabled: bool,
    /// The action types: 1 block the message, 2 send an alert, 3 time the
    /// member out, 4 block the member from interacting.
    pub actions: Vec<u8>,
    pub exempt_channels: usize,
    pub exempt_roles: usize,
    /// The words of a custom word rule, empty for the other kinds.
    pub keywords: Vec<String>,
}

impl AutoModRule {
    pub fn parse_list(body: &Value) -> Vec<Self> {
        body.as_array().map(|list| list.iter().filter_map(Self::parse).collect()).unwrap_or_default()
    }

    pub fn parse(value: &Value) -> Option<Self> {
        let size = |key: &str| value.get(key).and_then(Value::as_array).map_or(0, Vec::len);
        Some(Self {
            id: id(value, "id")?,
            name: text(value, "name").unwrap_or_default(),
            trigger: u8::try_from(count(value, "trigger_type")).unwrap_or(0),
            enabled: value.get("enabled").and_then(Value::as_bool).unwrap_or(false),
            actions: value
                .get("actions")
                .and_then(Value::as_array)
                .map(|list| list.iter().filter_map(|a| u8::try_from(a.get("type")?.as_u64()?).ok()).collect())
                .unwrap_or_default(),
            exempt_channels: size("exempt_channels"),
            exempt_roles: size("exempt_roles"),
            keywords: value
                .get("trigger_metadata")
                .and_then(|m| m.get("keyword_filter"))
                .and_then(Value::as_array)
                .map(|list| list.iter().filter_map(|w| w.as_str().map(str::to_owned)).collect())
                .unwrap_or_default(),
        })
    }

    /// The change that saves a new name and word list on a custom word rule.
    /// Only the two fields travel, so the rule's actions and exemptions stay.
    pub fn edit_body(name: &str, keywords: &[String]) -> Value {
        serde_json::json!({ "name": name, "trigger_metadata": { "keyword_filter": keywords } })
    }

    /// Whether the editor holds something other than what the server has.
    pub fn changed(&self, name: &str, keywords: &[String]) -> bool {
        name.trim() != self.name || (self.trigger == 1 && keywords != self.keywords)
    }

    /// What the rule watches for, in the words the web client uses.
    pub fn trigger_name(&self) -> &'static str {
        match self.trigger {
            1 => "Block Custom Words",
            3 => "Block Spam Content",
            4 => "Block Commonly Flagged Words",
            5 => "Block Mention Spam",
            6 => "Block Members Profile Content",
            _ => "Custom Rule",
        }
    }

    /// What the rule does, one short phrase per action.
    pub fn action_names(&self) -> Vec<&'static str> {
        self.actions
            .iter()
            .filter_map(|a| match a {
                1 => Some("Block Message"),
                2 => Some("Send Alert"),
                3 => Some("Timeout Member"),
                4 => Some("Block Interactions"),
                _ => None,
            })
            .collect()
    }
}

/// The words typed into the keyword box, split on commas and new lines, with
/// blanks dropped and repeats ignoring case.
pub fn parse_keywords(text: &str) -> Vec<String> {
    let mut words: Vec<String> = Vec::new();
    for word in text.split([',', '\n']).map(str::trim).filter(|w| !w.is_empty()) {
        if !words.iter().any(|w| w.eq_ignore_ascii_case(word)) {
            words.push(word.to_owned());
        }
    }
    words
}

/// The body that makes a custom word rule: it watches messages for the words
/// and blocks the ones that hold any. The rule starts on.
pub fn keyword_rule_body(name: &str, keywords: &[String]) -> Value {
    serde_json::json!({
        "name": name,
        "event_type": 1,
        "trigger_type": 1,
        "trigger_metadata": { "keyword_filter": keywords },
        "actions": [{ "type": 1, "metadata": {} }],
        "enabled": true,
        "exempt_channels": [],
        "exempt_roles": [],
    })
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
    fn webhooks_read_their_channel_and_build_their_link() {
        let list = WebhookEntry::parse_list(&json!([
            { "id": "5", "name": "Spidey Bot", "channel_id": "9", "token": "tok", "type": 1, "user": { "id": "1", "username": "ada" } },
            { "id": "6", "name": "Followed", "channel_id": "9", "type": 2,
              "user": { "id": "2", "username": "Wick", "discriminator": "3938" } },
            { "name": "no id" }
        ]));
        assert_eq!(list.len(), 2);
        assert_eq!(list[1].creator.as_deref(), Some("Wick#3938"));
        assert_eq!(list[0].channel, Some(Snowflake(9)));
        assert_eq!(list[0].creator.as_deref(), Some("ada"));
        assert_eq!(list[0].url().as_deref(), Some("https://discord.com/api/webhooks/5/tok"));
        assert_eq!(list[1].url(), None);
        assert_eq!(list[1].kind, 2);
    }

    #[test]
    fn automod_rules_read_their_trigger_actions_and_exemptions() {
        let list = AutoModRule::parse_list(&json!([
            { "id": "3", "name": "Bad words", "trigger_type": 1, "enabled": true,
              "trigger_metadata": { "keyword_filter": ["spam", "ads"] },
              "actions": [{ "type": 1 }, { "type": 2, "metadata": { "channel_id": "9" } }, { "type": 9 }],
              "exempt_channels": ["1", "2"], "exempt_roles": [] },
            { "id": "4", "trigger_type": 5 },
            { "name": "no id" }
        ]));
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].trigger_name(), "Block Custom Words");
        assert_eq!(list[0].action_names(), vec!["Block Message", "Send Alert"]);
        assert_eq!((list[0].exempt_channels, list[0].exempt_roles), (2, 0));
        assert!(list[0].enabled);
        assert_eq!(list[0].keywords, vec!["spam", "ads"]);
        assert!(list[1].keywords.is_empty());
        assert!(!list[1].enabled, "a rule that does not say is off");
        assert_eq!(list[1].trigger_name(), "Block Mention Spam");
        assert!(list[1].actions.is_empty());
    }

    #[test]
    fn keywords_are_split_trimmed_and_deduplicated() {
        assert_eq!(parse_keywords(" spam, Ads\nspam ,, SPAM "), vec!["spam", "Ads"]);
        assert!(parse_keywords(" , \n").is_empty());
    }

    #[test]
    fn a_new_keyword_rule_blocks_messages_and_starts_on() {
        let body = keyword_rule_body("No ads", &["ads".into(), "promo".into()]);
        assert_eq!(body["trigger_type"], 1);
        assert_eq!(body["event_type"], 1);
        assert_eq!(body["enabled"], true);
        assert_eq!(body["actions"][0]["type"], 1);
        assert_eq!(body["trigger_metadata"]["keyword_filter"], json!(["ads", "promo"]));
    }

    #[test]
    fn a_rule_edit_sends_only_the_name_and_words_and_knows_when_it_changed() {
        let rule = AutoModRule::parse(&json!({
            "id": "3", "name": "Ads", "trigger_type": 1, "trigger_metadata": { "keyword_filter": ["ads"] }
        }))
        .unwrap();
        assert_eq!(
            AutoModRule::edit_body("Promo", &["ads".into()]),
            json!({ "name": "Promo", "trigger_metadata": { "keyword_filter": ["ads"] } })
        );
        assert!(!rule.changed(" Ads ", &["ads".into()]));
        assert!(rule.changed("Ads", &["ads".into(), "x".into()]));
        assert!(rule.changed("Other", &["ads".into()]));
        let spam = AutoModRule::parse(&json!({ "id": "4", "name": "Spam", "trigger_type": 3 })).unwrap();
        assert!(!spam.changed("Spam", &["ignored".into()]), "only word rules own a word list");
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
