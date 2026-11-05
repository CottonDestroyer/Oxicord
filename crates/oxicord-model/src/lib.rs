//! Plain data types for the Discord API. No IO happens in this crate, so
//! everything here can be exercised from unit tests.

mod serde_util;

pub mod account;
pub mod attachment;
pub mod channel;
pub mod embed;
pub mod emoji;
pub mod gateway;
pub mod guild;
pub mod member;
pub mod message;
pub mod permissions;
pub mod presence;
pub mod read_state;
pub mod relationship;
pub mod role;
pub mod snowflake;
pub mod user;

pub use account::{AccountDetails, AppInfo, AuthorizedApp, ClientInfo, Connection, LoginSession};
pub use attachment::Attachment;
pub use channel::{Channel, ChannelType, Overwrite, OverwriteKind};
pub use embed::Embed;
pub use emoji::Emoji;
pub use guild::Guild;
pub use member::{Member, MemberList};
pub use message::{Message, MessageType, PartialMessage, Reaction};
pub use permissions::Permissions;
pub use presence::{Activity, Status};
pub use read_state::ReadState;
pub use relationship::{Relationship, RelationshipKind, RelationshipRemove};
pub use role::Role;
pub use snowflake::Snowflake;
pub use user::{Profile, User};
