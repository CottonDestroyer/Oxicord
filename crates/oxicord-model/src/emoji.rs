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
