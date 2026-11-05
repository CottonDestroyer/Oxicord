//! One file per resource. Each adds methods to `ApiClient`.

mod channels;
mod messages;
mod reactions;
mod relationships;
mod users;

pub use messages::{MessagePage, ReplyTarget, SendMessage};
pub use reactions::emoji_path_segment;
