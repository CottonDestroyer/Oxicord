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
