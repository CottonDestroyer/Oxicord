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

/// The extra details behind a profile card. Discord nests the bio under
/// `user_profile`, and older responses put it on the user, so take either.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(from = "RawProfile")]
pub struct Profile {
    pub bio: String,
    pub pronouns: String,
}

#[derive(Default, Deserialize)]
struct RawProfile {
    #[serde(default)]
    user_profile: Option<ProfileFields>,
    #[serde(default)]
    user: Option<ProfileFields>,
}

#[derive(Default, Deserialize)]
struct ProfileFields {
    #[serde(default)]
    bio: Option<String>,
    #[serde(default)]
    pronouns: Option<String>,
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
                .unwrap_or_default()
        };
        Self {
            bio: first(|s| s.bio.as_ref()),
            pronouns: first(|s| s.pronouns.as_ref()),
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
}
