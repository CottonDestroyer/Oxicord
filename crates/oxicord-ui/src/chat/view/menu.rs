//! The menu on a message: right click, or the "more" button on its toolbar.

use std::rc::Rc;

use gpui_kit::{ClipboardItem, Context, MouseDownEvent, Pixels, Point, Window};
use oxicord_model::Snowflake;

use super::{Allowed, ChatView};
use crate::{backend::Command, popup::Entry, session::{OpenForward, OpenPopup}};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Choice {
    React,
    Edit,
    Pin(bool),
    CopyText,
    Reply,
    Forward,
    MarkUnread,
    Thread,
    CopyLink,
    Bookmark(bool),
    Delete,
    CopyId,
    Separator,
}

impl Choice {
    fn entry(self) -> Entry {
        match self {
            Choice::React => Entry::item("Add Reaction", "face-slightly-smiling-plus"),
            Choice::Edit => Entry::item("Edit Message", "pencil"),
            Choice::Pin(true) => Entry::item("Unpin Message", "pin-off"),
            Choice::Pin(false) => Entry::item("Pin Message", "pin"),
            Choice::CopyText => Entry::item("Copy Text", "copy"),
            Choice::Reply => Entry::item("Reply", "reply"),
            Choice::Forward => Entry::item("Forward", "forward"),
            Choice::MarkUnread => Entry::item("Mark Unread", "mail"),
            Choice::Thread => Entry::item("Create Thread", "messages-square"),
            Choice::CopyLink => Entry::item("Copy Message Link", "link"),
            Choice::Bookmark(true) => Entry::item("Remove Bookmark", "bookmark"),
            Choice::Bookmark(false) => Entry::item("Bookmark Message", "bookmark"),
            Choice::Delete => Entry::danger("Delete Message", "trash"),
            Choice::CopyId => Entry::item("Copy Message ID", "hash"),
            Choice::Separator => Entry::Separator,
        }
    }
}

/// What the menu offers for a message, in the order shown.
pub(super) fn choices(allowed: &Allowed, bookmarked: bool) -> Vec<Choice> {
    let mut out = Vec::new();
    if allowed.react {
        out.push(Choice::React);
    }
    if allowed.edit {
        out.push(Choice::Edit);
    }
    if let Some(pinned) = allowed.pin {
        out.push(Choice::Pin(pinned));
    }
    out.push(Choice::CopyText);
    if allowed.reply {
        out.push(Choice::Reply);
    }
    out.push(Choice::Forward);
    out.push(Choice::MarkUnread);
    if allowed.thread {
        out.push(Choice::Thread);
    }
    out.push(Choice::Bookmark(bookmarked));
    out.push(Choice::CopyLink);
    if allowed.delete {
        out.push(Choice::Separator);
        out.push(Choice::Delete);
    }
    out.push(Choice::Separator);
    out.push(Choice::CopyId);
    out
}

impl ChatView {
    pub(super) fn open_message_menu(
        &mut self,
        message: Snowflake,
        at: Point<Pixels>,
        allowed: Allowed,
        cx: &mut Context<Self>,
    ) {
        let list = choices(&allowed, self.session.read(cx).is_bookmarked(message));
        let entries = list.iter().map(|c| c.entry()).collect();
        let chat = cx.entity().downgrade();
        let pick = Rc::new(move |index: usize, window: &mut Window, cx: &mut gpui_kit::App| {
            let Some(choice) = list.get(index).copied() else { return };
            chat.update(cx, |this, cx| this.pick_message_choice(message, choice, window, cx)).ok();
        });
        self.session.update(cx, |_, cx| cx.emit(OpenPopup { at, entries, pick }));
    }

    pub(super) fn right_click_message(
        &mut self,
        message: Snowflake,
        event: &MouseDownEvent,
        allowed: Allowed,
        cx: &mut Context<Self>,
    ) {
        self.open_message_menu(message, event.position, allowed, cx);
    }

    fn pick_message_choice(&mut self, message: Snowflake, choice: Choice, window: &mut Window, cx: &mut Context<Self>) {
        match choice {
            Choice::React => {
                self.picker_for = Some(message);
                cx.notify();
            }
            Choice::Edit => self.start_edit(message, window, cx),
            Choice::Pin(pinned) => self.set_pinned(message, !pinned, cx),
            Choice::CopyText => self.copy_text(message, cx),
            Choice::Reply => self.start_reply(message, window, cx),
            Choice::Forward => self.forward(message, cx),
            Choice::MarkUnread => self.mark_unread(message, cx),
            Choice::Thread => self.start_thread(message, cx),
            Choice::CopyLink => self.copy_link(message, cx),
            Choice::Bookmark(_) => self.toggle_bookmark(message, cx),
            Choice::Delete => {
                self.confirm_delete = Some(message);
                cx.notify();
            }
            Choice::CopyId => cx.write_to_clipboard(ClipboardItem::new_string(message.to_string())),
            Choice::Separator => {}
        }
    }

    pub(super) fn forward(&mut self, message: Snowflake, cx: &mut Context<Self>) {
        let Some(channel) = self.channel else { return };
        let guild = self.guild(cx);
        self.session.update(cx, |_, cx| cx.emit(OpenForward { channel, guild, message }));
    }

    fn toggle_bookmark(&mut self, message: Snowflake, cx: &mut Context<Self>) {
        let Some(channel) = self.channel else { return };
        let bookmarked = self.session.read(cx).is_bookmarked(message);
        if bookmarked {
            self.backend.send(Command::UnbookmarkMessage { channel, message });
            self.session.update(cx, |s, cx| s.set_bookmarked(message, false, cx));
        } else {
            self.backend.send(Command::BookmarkMessage { channel, message });
            self.session.update(cx, |s, cx| s.set_bookmarked(message, true, cx));
        }
    }
    fn mark_unread(&mut self, message: Snowflake, cx: &mut Context<Self>) {
        let Some(channel) = self.channel else { return };
        let (before, newest) = {
            let store = &self.session.read(cx).store;
            let timeline = store.timeline(channel);
            let before = timeline
                .and_then(|t| {
                    let list = t.messages();
                    let at = list.iter().position(|m| m.id == message)?;
                    at.checked_sub(1).map(|i| list[i].id)
                })
                // The first message we hold: any id just below it will do.
                .unwrap_or(Snowflake(message.0.saturating_sub(1)));
            (before, timeline.and_then(|t| t.newest_id()))
        };
        // Without this the open channel would mark itself read again at once.
        if let Some(newest) = newest {
            self.acked.insert(channel, newest);
        }
        self.backend.send(Command::MarkUnread { channel, message: before });
        self.session.update(cx, |session, cx| {
            let changes = session.store.mark_read(channel, before);
            cx.emit(changes);
            cx.notify();
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn allowed(edit: bool, delete: bool) -> Allowed {
        Allowed { reply: true, react: true, edit, delete, pin: None, thread: false }
    }

    #[test]
    fn your_own_message_can_be_edited_and_deleted() {
        let list = choices(&allowed(true, true), false);
        assert!(list.contains(&Choice::Edit) && list.contains(&Choice::Delete));
    }

    #[test]
    fn someone_elses_message_offers_neither() {
        let list = choices(&allowed(false, false), false);
        assert!(!list.contains(&Choice::Edit) && !list.contains(&Choice::Delete));
        assert!(list.contains(&Choice::MarkUnread) && list.contains(&Choice::CopyId));
    }

    #[test]
    fn the_pin_entry_reflects_the_current_state() {
        let mut a = allowed(false, false);
        a.pin = Some(true);
        assert!(choices(&a, false).contains(&Choice::Pin(true)));
    }
}
