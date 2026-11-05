use crate::Message;

/// One page of search hits.
#[derive(Clone, Debug, Default)]
pub struct SearchPage {
    /// Matches across all pages, not just this one.
    pub total: u64,
    pub messages: Vec<Message>,
}
