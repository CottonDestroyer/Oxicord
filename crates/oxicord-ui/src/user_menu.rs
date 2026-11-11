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
    /// Give a friend a personal nickname, shown only in our DMs.
    ChangeFriendNickname,
    CopyId,
    /// How loud they are in our call. The owner draws the slider itself,
    /// since it needs a state this list does not have.
    Volume,
    Separator,
}

impl Choice {
    pub fn entry(self) -> Entry {
        match self {
            Choice::Volume => Entry::Separator,
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
            Choice::ChangeFriendNickname => Entry::item("Change Friend Nickname", "user-edit"),
            Choice::CopyId => Entry::item("Copy User ID", "hash"),
            Choice::Separator => Entry::Separator,
        }
    }
}

/// What to offer for someone. `can_mention` is false where there is no
/// composer to put a mention in, `in_dm_list` adds closing the chat and
/// `in_our_call` adds their volume.
pub fn choices(
    tie: Option<RelationshipKind>,
    is_me: bool,
    can_mention: bool,
    in_dm_list: bool,
    in_our_call: bool,
) -> Vec<Choice> {
    let mut out = vec![Choice::Profile];
    if can_mention {
        out.push(Choice::Mention);
    }
    if !is_me {
        out.push(Choice::Message);
        if in_our_call {
            out.push(Choice::Separator);
            out.push(Choice::Volume);
        }
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
        // Only friends can carry a personal nickname.
        if tie == Some(RelationshipKind::Friend) {
            out.push(Choice::ChangeFriendNickname);
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
        let list = choices(None, false, true, false, false);
        assert!(list.contains(&Choice::AddFriend) && list.contains(&Choice::Block));
    }

    #[test]
    fn a_friend_can_be_removed_instead() {
        let list = choices(Some(RelationshipKind::Friend), false, true, false, false);
        assert!(list.contains(&Choice::RemoveFriend) && !list.contains(&Choice::AddFriend));
    }

    #[test]
    fn a_friend_can_have_their_nickname_changed() {
        let list = choices(Some(RelationshipKind::Friend), false, true, false, false);
        assert!(list.contains(&Choice::ChangeFriendNickname));
        assert!(!choices(None, false, true, false, false).contains(&Choice::ChangeFriendNickname));
    }

    #[test]
    fn someone_blocked_can_only_be_unblocked() {
        let list = choices(Some(RelationshipKind::Blocked), false, true, false, false);
        assert!(list.contains(&Choice::Unblock) && !list.contains(&Choice::Block));
    }

    #[test]
    fn you_get_no_social_actions_on_yourself() {
        let list = choices(None, true, true, false, false);
        assert!(!list.contains(&Choice::Message) && !list.contains(&Choice::AddFriend));
        assert!(list.contains(&Choice::Profile));
    }

    #[test]
    fn only_someone_in_our_call_has_a_volume() {
        assert!(choices(None, false, true, false, true).contains(&Choice::Volume));
        assert!(!choices(None, false, true, false, false).contains(&Choice::Volume));
        assert!(!choices(None, true, true, false, true).contains(&Choice::Volume), "not on yourself");
    }

    #[test]
    fn only_the_dm_list_offers_closing() {
        assert!(choices(None, false, false, true, false).contains(&Choice::CloseDm));
        assert!(!choices(None, false, false, false, false).contains(&Choice::CloseDm));
    }
}
