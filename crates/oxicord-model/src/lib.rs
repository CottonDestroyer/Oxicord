//! Plain data types for the Discord API. No IO happens in this crate, so
//! everything here can be exercised from unit tests.

mod serde_util;

pub mod account;
pub mod admin;
pub mod attachment;
pub mod channel;
pub mod command;
pub mod component;
pub mod effect;
pub mod frame;
pub mod embed;
pub mod emoji;
pub mod gateway;
pub mod gif;
pub mod guild;
pub mod invite;
pub mod member;
pub mod notification;
pub mod message;
pub mod permissions;
pub mod poll;
pub mod presence;
pub mod read_state;
pub mod relationship;
pub mod role;
pub mod search;
pub mod snowflake;
pub mod user;
pub mod verification;
pub mod voice;
pub mod welcome;

pub use account::{AccountDetails, AppInfo, AuthorizedApp, ClientInfo, Connection, LoginSession};
pub use attachment::Attachment;
pub use command::{Choice, CommandIndex, CommandOption, SlashCommand};
pub use component::{ButtonStyle, Component, FLAG_COMPONENTS_V2, MediaRef, SelectOption};
pub use channel::{
    Channel, ChannelType, FLAG_PINNED, ForumTag, Overwrite, OverwriteKind, ThreadMember, ThreadMetadata,
};
pub use effect::{EffectLayer, ProfileEffect};
pub use frame::{FrameLayer, ProfileFrame};
pub use embed::Embed;
pub use emoji::Emoji;
pub use gif::Gif;
pub use guild::Guild;
pub use invite::{InvitePreview, invite_code};
pub use member::{Member, MemberList};
pub use notification::{
    ChannelOverride, GuildNotificationSettings, MuteConfig, NotificationSettingsList, NotifyLevel,
    mute_active,
};
pub use message::{
    Message, MessageSnapshot, MessageType, PartialMessage, Reaction, SavedMessage, SavedMessageData,
    SnapshotMessage, StickerItem,
};
pub use permissions::Permissions;
pub use poll::{Poll, PollAnswer, Tally};
pub use presence::{Activity, ActivityTimestamps, ActivityType, Status};
pub use read_state::ReadState;
pub use relationship::{Relationship, RelationshipKind, RelationshipRemove};
pub use role::Role;
pub use search::{SearchPage, SearchQuery, ThreadPage};
pub use snowflake::Snowflake;
pub use user::{AccountMetadata, Badge, Game, GameEntry, LinkedAccount, MutualGuild, Profile, ServerTag, User};
pub use verification::{FormField, GuildProfile, VerificationForm, VerificationInfo, VerificationResponse};
pub use welcome::{WelcomeChannel, WelcomeScreen};
pub use voice::VoiceState;
