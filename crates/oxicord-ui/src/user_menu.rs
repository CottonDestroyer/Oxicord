//! The menu for a person, opened by right clicking a name, an avatar, a
//! member or a conversation. What it offers depends on who they are to us.

use oxicord_model::RelationshipKind;

use crate::popup::Entry;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
    Profile,
    Mention,
    Message,
    AddFriend,
    RemoveFriend,
    AcceptRequest,
    CancelRequest,
    Block,
    Unblock,
    /// Hide the conversation, for the DM list.
    CloseDm,
    CopyId,
    Separator,
}

impl Choice {
    pub fn entry(self) -> Entry {
        match self {
            Choice::Profile => Entry::item("Profile", "user"),
            Choice::Mention => Entry::item("Mention", "at-sign"),
            Choice::Message => Entry::item("Message", "message-circle"),
            Choice::AddFriend => Entry::item("Add Friend", "user-plus"),
            Choice::RemoveFriend => Entry::danger("Remove Friend", "user-minus"),
            Choice::AcceptRequest => Entry::item("Accept Friend Request", "user-check"),
            Choice::CancelRequest => Entry::danger("Cancel Friend Request", "user-x"),
            Choice::Block => Entry::danger("Block", "ban"),
            Choice::Unblock => Entry::item("Unblock", "ban"),
            Choice::CloseDm => Entry::item("Close DM", "x"),
            Choice::CopyId => Entry::item("Copy User ID", "hash"),
            Choice::Separator => Entry::Separator,
        }
    }
}

/// What to offer for someone. `can_mention` is false where there is no
/// composer to put a mention in, and `in_dm_list` adds closing the chat.
pub fn choices(tie: Option<RelationshipKind>, is_me: bool, can_mention: bool, in_dm_list: bool) -> Vec<Choice> {
    let mut out = vec![Choice::Profile];
    if can_mention {
        out.push(Choice::Mention);
    }
    if !is_me {
        out.push(Choice::Message);
        out.push(Choice::Separator);
        out.push(match tie {
            Some(RelationshipKind::Friend) => Choice::RemoveFriend,
            Some(RelationshipKind::IncomingRequest) => Choice::AcceptRequest,
            Some(RelationshipKind::OutgoingRequest) => Choice::CancelRequest,
            Some(RelationshipKind::Blocked) => Choice::Unblock,
            _ => Choice::AddFriend,
        });
        // Blocking someone already blocked is what Unblock is for.
        if tie != Some(RelationshipKind::Blocked) {
            out.push(Choice::Block);
        }
    }
    if in_dm_list {
        out.push(Choice::Separator);
        out.push(Choice::CloseDm);
    }
    out.push(Choice::Separator);
    out.push(Choice::CopyId);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stranger_can_be_added_or_blocked() {
        let list = choices(None, false, true, false);
        assert!(list.contains(&Choice::AddFriend) && list.contains(&Choice::Block));
    }

    #[test]
    fn a_friend_can_be_removed_instead() {
        let list = choices(Some(RelationshipKind::Friend), false, true, false);
        assert!(list.contains(&Choice::RemoveFriend) && !list.contains(&Choice::AddFriend));
    }

    #[test]
    fn someone_blocked_can_only_be_unblocked() {
        let list = choices(Some(RelationshipKind::Blocked), false, true, false);
        assert!(list.contains(&Choice::Unblock) && !list.contains(&Choice::Block));
    }

    #[test]
    fn you_get_no_social_actions_on_yourself() {
        let list = choices(None, true, true, false);
        assert!(!list.contains(&Choice::Message) && !list.contains(&Choice::AddFriend));
        assert!(list.contains(&Choice::Profile));
    }

    #[test]
    fn only_the_dm_list_offers_closing() {
        assert!(choices(None, false, false, true).contains(&Choice::CloseDm));
        assert!(!choices(None, false, false, false).contains(&Choice::CloseDm));
    }
}
