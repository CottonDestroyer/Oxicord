//! One file per resource. Each adds methods to `ApiClient`.

mod account;
mod channels;
mod messages;
mod reactions;
mod search;
mod relationships;
mod users;

pub use search::{SEARCH_PAGE, SearchOutcome, SearchScope};
pub use messages::{MessagePage, ReplyTarget, SendMessage, Upload};
pub use reactions::emoji_path_segment;
