use crate::Message;

/// One page of search hits.
#[derive(Clone, Debug, Default)]
pub struct SearchPage {
    /// Matches across all pages, not just this one.
    pub total: u64,
    pub messages: Vec<Message>,
}

/// A page of threads from a channel's thread search, with what each one
/// shows in a list.
#[derive(Clone, Debug, Default)]
pub struct ThreadPage {
    pub threads: Vec<crate::Channel>,
    /// Our own memberships among them.
    pub joined: Vec<crate::Snowflake>,
    pub has_more: bool,
    /// The opening message of each thread, which a forum shows as a preview.
    pub first_messages: Vec<Message>,
}

/// What a search asks for: words plus the filters Discord's search box
/// understands (`from:`, `in:`, `has:`, `mentions:`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SearchQuery {
    pub content: String,
    pub authors: Vec<crate::Snowflake>,
    pub channels: Vec<crate::Snowflake>,
    pub mentions: Vec<crate::Snowflake>,
    /// `image`, `video`, `file`, `link`, `embed`, `sound` or `sticker`.
    pub has: Vec<String>,
}

impl SearchQuery {
    /// Nothing to look for at all.
    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
            && self.authors.is_empty()
            && self.channels.is_empty()
            && self.mentions.is_empty()
            && self.has.is_empty()
    }
}
