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
    #[serde(default)]
    pub nsfw: bool,
    #[serde(default)]
    pub owner_id: Option<Snowflake>,
    #[serde(default)]
    pub icon: Option<String>,
}
