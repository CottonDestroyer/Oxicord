use serde::Deserialize;
use serde_repr::Deserialize_repr;

use crate::{Snowflake, User};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Deserialize_repr)]
#[repr(u8)]
pub enum RelationshipKind {
    #[default]
    #[serde(other)]
    None = 0,
    Friend = 1,
    Blocked = 2,
    IncomingRequest = 3,
    OutgoingRequest = 4,
    /// Someone Discord suggests from mutual servers. Not a real tie, so the
    /// friends page leaves these out.
    Implicit = 5,
}

/// One entry of the friends list. `id` is the other person's user id.
#[derive(Clone, Debug, Deserialize)]
pub struct Relationship {
    pub id: Snowflake,
    #[serde(rename = "type", default)]
    pub kind: RelationshipKind,
    /// READY may leave this out and list the user in its top level `users`
    /// instead, so the store joins them.
    #[serde(default)]
    pub user: Option<User>,
    #[serde(default)]
    pub nickname: Option<String>,
}

/// RELATIONSHIP_REMOVE carries just enough to say which tie went away.
#[derive(Clone, Debug, Deserialize)]
pub struct RelationshipRemove {
    pub id: Snowflake,
}
