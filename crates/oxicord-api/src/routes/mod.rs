//! One file per resource. Each adds methods to `ApiClient`.

mod account;
mod admin;
mod channels;
mod commands;
mod gifs;
mod guilds;
mod interactions;
mod invites;
mod messages;
mod reactions;
mod search;
mod threads;
mod relationships;
mod users;

pub use search::{SEARCH_PAGE, SearchOutcome, SearchScope};
pub use interactions::{ComponentClick, ModalSubmit, click_body, modal_body};
pub use commands::{CommandRun, IndexOutcome, run_body};
pub use messages::{AllowedMentions, MessagePage, PollAnswerCreate, PollCreate, PollMedia, ReplyTarget, SendMessage, Upload};
pub use reactions::emoji_path_segment;
