//! Discord flavored markdown, parsed into a tree the UI can draw.
//!
//! Discord's dialect is looser than CommonMark. Emphasis doesn't need
//! surrounding spaces, `_` doesn't italicize inside a word, newlines always
//! break lines, and there are extra forms: spoilers, `<@id>` mentions,
//! custom emoji and timestamps. Text that doesn't parse as markup stays
//! text, so malformed input never gets lost or turned into an error.

mod ast;
mod block;
mod inline;

pub use ast::{Block, Inline};
pub use block::parse;
pub use inline::parse_inline;
