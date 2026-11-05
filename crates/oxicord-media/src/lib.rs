//! Everything about fetching pictures: where Discord keeps them, whether a
//! URL is safe to fetch, and the fetcher itself.

pub mod cdn;
mod fetch;
mod sniff;

pub use fetch::{FetchError, Fetcher, is_allowed};
pub use sniff::{ImageKind, sniff};
