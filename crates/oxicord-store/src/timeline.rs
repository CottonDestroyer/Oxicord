use std::collections::VecDeque;

use oxicord_model::{Message, Snowflake};

/// Live messages are trimmed from the old end past this size, so a channel
/// left open for days doesn't grow without bound.
const MAX_LIVE_LEN: usize = 1000;

/// The messages we hold for one channel, oldest first.
#[derive(Clone, Debug, Default)]
pub struct Timeline {
    messages: VecDeque<Message>,
    /// True while older history may still exist on the server.
    has_more_before: bool,
    /// Holds a stretch of history around a message we jumped to, not the
    /// newest messages. Live messages are not added while this is set,
    /// because they would sit apart from what is shown.
    detached: bool,
}

impl Timeline {
    pub fn messages(&self) -> &VecDeque<Message> {
        &self.messages
    }

    pub fn is_detached(&self) -> bool {
        self.detached
    }

    pub(crate) fn set_detached(&mut self, value: bool) {
        self.detached = value;
    }

    pub fn has_more_before(&self) -> bool {
        self.has_more_before
    }

    pub fn oldest_id(&self) -> Option<Snowflake> {
        self.messages.front().map(|m| m.id)
    }

    pub fn newest_id(&self) -> Option<Snowflake> {
        self.messages.back().map(|m| m.id)
    }

    pub fn get(&self, id: Snowflake) -> Option<&Message> {
        self.index_of(id).ok().map(|i| &self.messages[i])
    }

    pub(crate) fn get_mut(&mut self, id: Snowflake) -> Option<&mut Message> {
        self.index_of(id).ok().map(|i| &mut self.messages[i])
    }

    /// Inserts in id order, replacing a message that is already there.
    pub(crate) fn insert(&mut self, message: Message) {
        match self.index_of(message.id) {
            Ok(i) => self.messages[i] = message,
            Err(i) => self.messages.insert(i, message),
        }
    }

    pub(crate) fn insert_live(&mut self, message: Message) {
        self.insert(message);
        while self.messages.len() > MAX_LIVE_LEN {
            self.messages.pop_front();
            self.has_more_before = true;
        }
    }

    pub(crate) fn remove(&mut self, id: Snowflake) -> bool {
        match self.index_of(id) {
            Ok(i) => {
                self.messages.remove(i);
                true
            }
            Err(_) => false,
        }
    }

    pub(crate) fn set_has_more_before(&mut self, value: bool) {
        self.has_more_before = value;
    }

    fn index_of(&self, id: Snowflake) -> Result<usize, usize> {
        self.messages.binary_search_by_key(&id, |m| m.id)
    }
}
