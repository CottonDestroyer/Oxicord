use serde::Deserialize;

use crate::{Snowflake, User};

/// `user` is absent when the member rides along on a message, because the
/// author is already on the message itself.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Member {
    #[serde(default)]
    pub user: Option<User>,
    #[serde(default)]
    pub nick: Option<String>,
    #[serde(default)]
    pub avatar: Option<String>,
    #[serde(default)]
    pub roles: Vec<Snowflake>,
    #[serde(default)]
    pub joined_at: Option<String>,
    #[serde(default)]
    pub communication_disabled_until: Option<String>,
}
