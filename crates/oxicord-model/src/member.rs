use serde::Deserialize;

use crate::{Snowflake, User};

/// Members show up in a few shapes:
///
/// - On a message, `user` is absent because the author is on the message.
/// - In READY, `merged_members` carry a bare `user_id` instead of a `user`,
///   with the full users listed once at the top level.
///
/// Use [`Member::id`] to get the user id whichever shape this is.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Member {
    #[serde(default)]
    pub user: Option<User>,
    #[serde(default)]
    pub user_id: Option<Snowflake>,
    #[serde(default)]
    pub nick: Option<String>,
    #[serde(default)]
    pub avatar: Option<String>,
    #[serde(default)]
    pub banner: Option<String>,
    #[serde(default)]
    pub roles: Vec<Snowflake>,
    #[serde(default)]
    pub joined_at: Option<String>,
    #[serde(default)]
    pub communication_disabled_until: Option<String>,
    /// True until we pass the server's membership screening.
    #[serde(default)]
    pub pending: bool,
}

impl Member {
    pub fn id(&self) -> Option<Snowflake> {
        self.user.as_ref().map(|u| u.id).or(self.user_id)
    }
}

/// READY lists the members of each guild, but it is not obvious from the
/// payload whether that is an array or one object. Accept both.
#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum MemberList {
    Many(Vec<Member>),
    One(Box<Member>),
}

impl MemberList {
    pub fn into_vec(self) -> Vec<Member> {
        match self {
            Self::Many(list) => list,
            Self::One(member) => vec![*member],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_comes_from_user_or_bare_user_id() {
        let with_user: Member =
            serde_json::from_str(r#"{"user":{"id":"5","username":"a"}}"#).unwrap();
        let bare: Member = serde_json::from_str(r#"{"user_id":"6","roles":["1"]}"#).unwrap();
        assert_eq!(with_user.id(), Some(Snowflake(5)));
        assert_eq!(bare.id(), Some(Snowflake(6)));
    }

    #[test]
    fn member_list_accepts_array_or_single_object() {
        let many: MemberList = serde_json::from_str(r#"[{"user_id":"1"},{"user_id":"2"}]"#).unwrap();
        let one: MemberList = serde_json::from_str(r#"{"user_id":"1"}"#).unwrap();
        assert_eq!(many.into_vec().len(), 2);
        assert_eq!(one.into_vec().len(), 1);
    }
}
