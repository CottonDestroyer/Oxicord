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
