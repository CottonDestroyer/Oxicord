use serde::Deserialize;

use crate::Snowflake;

/// Unicode emoji have a name and no id. Custom emoji have both.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
pub struct Emoji {
    #[serde(default)]
    pub id: Option<Snowflake>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub animated: bool,
}

impl Emoji {
    /// What tells one reaction from another on a message: the character, or
    /// `name:id` for a custom emoji.
    pub fn key(&self) -> String {
        let name = self.name.as_deref().unwrap_or_default();
        match self.id {
            Some(id) => format!("{name}:{id}"),
            None => name.to_owned(),
        }
    }
}
