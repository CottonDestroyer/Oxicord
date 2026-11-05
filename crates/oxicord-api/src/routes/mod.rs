//! One file per resource. Each adds methods to `ApiClient`.

mod account;
mod channels;
mod interactions;
mod messages;
mod reactions;
mod search;
mod threads;
mod relationships;
mod users;

pub use search::{SEARCH_PAGE, SearchOutcome, SearchScope};
pub use interactions::{ComponentClick, click_body};
pub use messages::{MessagePage, ReplyTarget, SendMessage, Upload};
pub use reactions::emoji_path_segment;
