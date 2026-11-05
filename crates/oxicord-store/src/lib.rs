//! Client side state, built up from the READY snapshot and kept current by
//! gateway events. Nothing here knows about the UI or the network, so the
//! whole reducer can be tested with plain values.

mod apply;
mod changes;
mod member_list;
mod notifications;
pub mod perms;
mod state;
mod threads;
mod timeline;

pub use changes::Changes;
pub use member_list::{Entry, MemberListState};
pub use state::{ChannelGroup, Store, Unread};
pub use timeline::Timeline;
