use serde::Deserialize;

use crate::Snowflake;

#[derive(Clone, Debug, Deserialize)]
pub struct User {
    pub id: Snowflake,
    #[serde(default)]
    pub username: String,
    /// "0" for accounts migrated to unique usernames.
    #[serde(default)]
    pub discriminator: Option<String>,
    #[serde(default)]
    pub global_name: Option<String>,
    #[serde(default)]
    pub avatar: Option<String>,
    #[serde(default)]
    pub bot: bool,
}

impl User {
    pub fn display_name(&self) -> &str {
        self.global_name
            .as_deref()
            .filter(|n| !n.is_empty())
            .unwrap_or(&self.username)
    }
}

/// A badge shown next to a name, such as an early supporter mark.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
pub struct Badge {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub description: String,
    /// The file name under the badge icon path on the CDN.
    #[serde(default)]
    pub icon: String,
}

/// A server both of us are in.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct MutualGuild {
    pub id: crate::Snowflake,
    #[serde(default)]
    pub nick: Option<String>,
}

/// An account linked on the profile, like a game or social site.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct LinkedAccount {
    #[serde(rename = "type", default)]
    pub kind: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub verified: bool,
}

/// The extra details behind a profile card. Discord nests the bio under
/// `user_profile`, and older responses put it on the user, so take either.
/// Everything is optional: a profile we only half understand still shows.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Profile {
    pub bio: String,
    pub pronouns: String,
    pub banner: Option<String>,
    pub accent_color: Option<u32>,
    pub server_tag: Option<String>,
    pub badges: Vec<Badge>,
    pub mutual_guilds: Vec<MutualGuild>,
    pub mutual_friends: u32,
    pub accounts: Vec<LinkedAccount>,
    /// When they started boosting or subscribed, as sent.
    pub premium_since: Option<String>,
    /// When they joined the server the card was opened from.
    pub joined_server: Option<String>,
}

#[derive(Default, Deserialize)]
struct RawProfile {
    #[serde(default)]
    user_profile: Option<ProfileFields>,
    #[serde(default)]
    user: Option<ProfileFields>,
    #[serde(default, deserialize_with = "crate::serde_util::lenient_vec")]
    badges: Vec<Badge>,
    #[serde(default, deserialize_with = "crate::serde_util::lenient_vec")]
    mutual_guilds: Vec<MutualGuild>,
    #[serde(default)]
    mutual_friends_count: Option<u32>,
    #[serde(default, deserialize_with = "crate::serde_util::lenient_vec")]
    connected_accounts: Vec<LinkedAccount>,
    #[serde(default)]
    premium_since: Option<String>,
    #[serde(default)]
    guild_member: Option<GuildMemberFields>,
    #[serde(default)]
    primary_guild: Option<PrimaryGuildFields>,
}

#[derive(Default, Deserialize)]
struct GuildMemberFields {
    #[serde(default)]
    joined_at: Option<String>,
}

#[derive(Default, Deserialize)]
struct PrimaryGuildFields {
    #[serde(default)]
    tag: Option<String>,
    #[serde(default)]
    identity_enabled: Option<bool>,
}

#[derive(Default, Deserialize)]
struct ProfileFields {
    #[serde(default)]
    bio: Option<String>,
    #[serde(default)]
    pronouns: Option<String>,
    #[serde(default)]
    banner: Option<String>,
    #[serde(default)]
    accent_color: Option<u32>,
}

impl<'de> Deserialize<'de> for Profile {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        // One odd field must not lose the whole profile, so read loosely.
        let value = serde_json::Value::deserialize(d)?;
        Ok(serde_json::from_value::<RawProfile>(value).unwrap_or_default().into())
    }
}

impl From<RawProfile> for Profile {
    fn from(raw: RawProfile) -> Self {
        let sources = [raw.user_profile, raw.user];
        let first = |field: fn(&ProfileFields) -> Option<&String>| {
            sources
                .iter()
                .flatten()
                .find_map(|s| field(s).filter(|v| !v.is_empty()))
                .cloned()
        };
        Self {
            bio: first(|s| s.bio.as_ref()).unwrap_or_default(),
            pronouns: first(|s| s.pronouns.as_ref()).unwrap_or_default(),
            banner: first(|s| s.banner.as_ref()),
            accent_color: sources.iter().flatten().find_map(|s| s.accent_color),
            badges: raw.badges,
            mutual_guilds: raw.mutual_guilds,
            mutual_friends: raw.mutual_friends_count.unwrap_or(0),
            accounts: raw.connected_accounts,
            premium_since: raw.premium_since,
            joined_server: raw.guild_member.and_then(|m| m.joined_at),
            server_tag: raw.primary_guild.and_then(|g| {
                if g.identity_enabled == Some(false) {
                    None
                } else {
                    g.tag.filter(|t| !t.is_empty())
                }
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_prefers_the_nested_bio_and_tolerates_gaps() {
        let nested: Profile = serde_json::from_str(
            r#"{"user":{"id":"1","bio":"old"},"user_profile":{"bio":"hello","pronouns":"they/them"}}"#,
        )
        .unwrap();
        assert_eq!(nested.bio, "hello");
        assert_eq!(nested.pronouns, "they/them");

        let flat: Profile = serde_json::from_str(r#"{"user":{"bio":"just this"}}"#).unwrap();
        assert_eq!(flat.bio, "just this");
        assert_eq!(serde_json::from_str::<Profile>("{}").unwrap(), Profile::default());
    }

    #[test]
    fn the_richer_fields_are_read_and_bad_ones_dropped() {
        let p: Profile = serde_json::from_str(
            r#"{"user":{"id":"1","accent_color":255},"user_profile":{"bio":"hi","banner":"abc"},
                "badges":[{"id":"x","description":"Early","icon":"i1"},"junk"],
                "mutual_guilds":[{"id":"5","nick":"N"},{"id":"bad"}],"mutual_friends_count":3,
                "connected_accounts":[{"type":"steam","name":"me","verified":true}],
                "premium_since":"2020-01-01","guild_member":{"joined_at":"2021-02-03"}}"#,
        )
        .unwrap();
        assert_eq!(p.banner.as_deref(), Some("abc"));
        assert_eq!(p.accent_color, Some(255));
        assert_eq!(p.badges.len(), 1);
        assert_eq!(p.mutual_guilds.len(), 1);
        assert_eq!(p.mutual_friends, 3);
        assert_eq!(p.accounts[0].name, "me");
        assert_eq!(p.joined_server.as_deref(), Some("2021-02-03"));
    }

}
