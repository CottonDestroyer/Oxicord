use serde::Deserialize;

use crate::Snowflake;

/// What an invite code points at, as shown before joining.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InvitePreview {
    pub code: String,
    pub guild_id: Option<Snowflake>,
    pub guild_name: String,
    pub icon: Option<String>,
    pub channel_name: Option<String>,
    pub members: Option<u64>,
    pub online: Option<u64>,
}

#[derive(Deserialize)]
struct Raw {
    #[serde(default)]
    guild: Option<RawGuild>,
    #[serde(default)]
    channel: Option<RawChannel>,
    #[serde(default)]
    approximate_member_count: Option<u64>,
    #[serde(default)]
    approximate_presence_count: Option<u64>,
}

#[derive(Deserialize)]
struct RawGuild {
    id: Snowflake,
    #[serde(default)]
    name: String,
    #[serde(default)]
    icon: Option<String>,
}

#[derive(Deserialize)]
struct RawChannel {
    #[serde(default)]
    name: Option<String>,
}

impl InvitePreview {
    /// Reads the answer of an invite lookup. `None` when it names no server,
    /// as group DM invites don't.
    pub fn from_json(code: &str, body: &[u8]) -> Option<Self> {
        let raw: Raw = serde_json::from_slice(body).ok()?;
        let guild = raw.guild?;
        Some(Self {
            code: code.to_owned(),
            guild_id: Some(guild.id),
            guild_name: guild.name,
            icon: guild.icon,
            channel_name: raw.channel.and_then(|c| c.name),
            members: raw.approximate_member_count,
            online: raw.approximate_presence_count,
        })
    }
}

/// Pulls the code out of whatever was pasted: a full link, a short link or
/// the bare code.
pub fn invite_code(text: &str) -> Option<String> {
    let text = text.trim().trim_end_matches('/');
    let candidate = text
        .rsplit_once('/')
        .map_or(text, |(_, last)| last)
        .split(['?', '#'])
        .next()
        .unwrap_or_default();
    let valid = !candidate.is_empty()
        && candidate.len() <= 32
        && candidate.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
    valid.then(|| candidate.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_come_out_of_every_link_shape() {
        for text in [
            "abc123",
            "https://discord.gg/abc123",
            "discord.gg/abc123",
            "https://discord.com/invite/abc123",
            "  https://discord.gg/abc123/  ",
            "https://discord.gg/abc123?event=1",
        ] {
            assert_eq!(invite_code(text).as_deref(), Some("abc123"), "{text}");
        }
        assert_eq!(invite_code(""), None);
        assert_eq!(invite_code("not a code!"), None);
        assert_eq!(invite_code("https://discord.gg/"), None);
    }

    #[test]
    fn a_server_invite_reads_its_preview() {
        let body = br#"{"code":"x","guild":{"id":"5","name":"Cats","icon":"h"},
            "channel":{"name":"general"},"approximate_member_count":100,"approximate_presence_count":20}"#;
        let preview = InvitePreview::from_json("x", body).unwrap();
        assert_eq!(preview.guild_name, "Cats");
        assert_eq!((preview.members, preview.online), (Some(100), Some(20)));
        assert!(InvitePreview::from_json("x", br#"{"channel":{"name":"gdm"}}"#).is_none());
    }
}
