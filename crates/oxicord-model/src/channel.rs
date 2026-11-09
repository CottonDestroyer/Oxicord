use serde::Deserialize;
use serde_repr::Deserialize_repr;

use crate::{Permissions, Snowflake, User};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Deserialize_repr)]
#[repr(u8)]
pub enum ChannelType {
    #[default]
    GuildText = 0,
    Dm = 1,
    GuildVoice = 2,
    GroupDm = 3,
    GuildCategory = 4,
    GuildAnnouncement = 5,
    AnnouncementThread = 10,
    PublicThread = 11,
    PrivateThread = 12,
    GuildStageVoice = 13,
    GuildDirectory = 14,
    GuildForum = 15,
    GuildMedia = 16,
    #[serde(other)]
    Unknown = 255,
}

impl ChannelType {
    pub fn is_thread(self) -> bool {
        matches!(
            self,
            Self::AnnouncementThread | Self::PublicThread | Self::PrivateThread
        )
    }

    pub fn is_dm(self) -> bool {
        matches!(self, Self::Dm | Self::GroupDm)
    }

    /// Channels that hold a plain message timeline.
    pub fn has_messages(self) -> bool {
        matches!(
            self,
            Self::GuildText
                | Self::Dm
                | Self::GroupDm
                | Self::GuildAnnouncement
                | Self::GuildVoice
                | Self::GuildStageVoice
        ) || self.is_thread()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize_repr)]
#[repr(u8)]
pub enum OverwriteKind {
    Role = 0,
    Member = 1,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Overwrite {
    pub id: Snowflake,
    #[serde(rename = "type")]
    pub kind: OverwriteKind,
    #[serde(default)]
    pub allow: Permissions,
    #[serde(default)]
    pub deny: Permissions,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Channel {
    pub id: Snowflake,
    #[serde(rename = "type")]
    pub kind: ChannelType,
    /// Missing on guild channels delivered inside GUILD_CREATE, where the
    /// parent guild is implied. `Guild` fills it in.
    #[serde(default)]
    pub guild_id: Option<Snowflake>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub topic: Option<String>,
    #[serde(default)]
    pub position: i32,
    #[serde(default)]
    pub parent_id: Option<Snowflake>,
    #[serde(default)]
    pub permission_overwrites: Vec<Overwrite>,
    #[serde(default)]
    pub last_message_id: Option<Snowflake>,
    #[serde(default)]
    pub recipients: Vec<User>,
    #[serde(default)]
    pub recipient_ids: Vec<Snowflake>,
    /// A DM from someone we have not accepted a conversation with yet. The
    /// client keeps these out of the DM list until they are accepted.
    #[serde(default)]
    pub is_message_request: bool,
    /// When the request arrived. The request list sorts by the newer of this
    /// and `last_message_id`.
    #[serde(default)]
    pub is_message_request_timestamp: Option<String>,
    /// A message request Discord sorted into the spam tab.
    #[serde(default)]
    pub is_spam: bool,
    #[serde(default)]
    pub nsfw: bool,
    #[serde(default)]
    pub owner_id: Option<Snowflake>,
    #[serde(default)]
    pub icon: Option<String>,
    /// Present on threads and forum posts.
    #[serde(default)]
    pub thread_metadata: Option<ThreadMetadata>,
    /// Our own membership of a thread, absent when we haven't joined.
    #[serde(default)]
    pub member: Option<ThreadMember>,
    #[serde(default)]
    pub message_count: Option<u32>,
    #[serde(default)]
    pub member_count: Option<u32>,
    /// Tags on a forum post, as ids into the forum's `available_tags`.
    #[serde(default)]
    pub applied_tags: Vec<Snowflake>,
    /// The tags a forum offers.
    #[serde(default)]
    pub available_tags: Vec<ForumTag>,
    #[serde(default)]
    pub flags: u64,
    /// The text people set on a voice channel while they sit in it.
    #[serde(default)]
    pub status: Option<String>,
    /// User limit for voice channels (None = unlimited).
    #[serde(default)]
    pub user_limit: Option<u32>,
}

/// Whether a thread is still open. An archived thread is hidden but can be
/// reopened by posting in it.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct ThreadMetadata {
    #[serde(default)]
    pub archived: bool,
    #[serde(default)]
    pub locked: bool,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ThreadMember {
    #[serde(default)]
    pub flags: u64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ForumTag {
    pub id: Snowflake,
    pub name: String,
    #[serde(default)]
    pub emoji_name: Option<String>,
}

/// Channel flag bit that pins a forum post to the top of its list.
pub const FLAG_PINNED: u64 = 1 << 1;

impl Channel {
    /// The client's rule for the requests list: flagged as a message request
    /// and not sorted into spam.
    pub fn is_request(&self) -> bool {
        self.is_message_request && !self.is_spam
    }

    pub fn archived(&self) -> bool {
        self.thread_metadata.as_ref().is_some_and(|m| m.archived)
    }

    pub fn locked(&self) -> bool {
        self.thread_metadata.as_ref().is_some_and(|m| m.locked)
    }

    pub fn joined(&self) -> bool {
        self.member.is_some()
    }

    pub fn pinned_post(&self) -> bool {
        self.flags & FLAG_PINNED != 0
    }
}
