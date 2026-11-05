use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_repr::Deserialize_repr;

use crate::{Attachment, Embed, Emoji, Member, Snowflake, User};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize_repr)]
#[repr(u8)]
pub enum MessageType {
    #[default]
    Default = 0,
    RecipientAdd = 1,
    RecipientRemove = 2,
    Call = 3,
    ChannelNameChange = 4,
    ChannelIconChange = 5,
    ChannelPinnedMessage = 6,
    UserJoin = 7,
    Reply = 19,
    ChatInputCommand = 20,
    ThreadStarterMessage = 21,
    ContextMenuCommand = 23,
    #[serde(other)]
    Unknown = 255,
}

impl MessageType {
    /// Types that carry text the user wrote, as opposed to system notices.
    pub fn is_regular(self) -> bool {
        matches!(
            self,
            Self::Default | Self::Reply | Self::ChatInputCommand | Self::ContextMenuCommand
        )
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct MessageReference {
    #[serde(default)]
    pub message_id: Option<Snowflake>,
    #[serde(default)]
    pub channel_id: Option<Snowflake>,
    #[serde(default)]
    pub guild_id: Option<Snowflake>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Reaction {
    pub count: u32,
    #[serde(default)]
    pub me: bool,
    pub emoji: Emoji,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Message {
    pub id: Snowflake,
    pub channel_id: Snowflake,
    #[serde(default)]
    pub guild_id: Option<Snowflake>,
    pub author: User,
    #[serde(default)]
    pub member: Option<Member>,
    #[serde(default)]
    pub content: String,
    pub timestamp: DateTime<Utc>,
    #[serde(default)]
    pub edited_timestamp: Option<DateTime<Utc>>,
    #[serde(default)]
    pub mention_everyone: bool,
    #[serde(default)]
    pub mentions: Vec<User>,
    #[serde(default)]
    pub mention_roles: Vec<Snowflake>,
    #[serde(default)]
    pub attachments: Vec<Attachment>,
    #[serde(default)]
    pub embeds: Vec<Embed>,
    #[serde(default)]
    pub reactions: Vec<Reaction>,
    #[serde(default)]
    pub pinned: bool,
    #[serde(rename = "type", default)]
    pub kind: MessageType,
    /// The thread started from this message, if any.
    #[serde(default)]
    pub thread: Option<Box<crate::Channel>>,
    #[serde(default)]
    pub message_reference: Option<MessageReference>,
    #[serde(default)]
    pub referenced_message: Option<Box<Message>>,
    #[serde(default)]
    pub flags: u64,
    /// Echoed back from the send request so we can match our own optimistic
    /// message against the one the gateway delivers.
    #[serde(default)]
    pub nonce: Option<serde_json::Value>,
}

/// MESSAGE_UPDATE is often partial. Link unfurling only sends the id,
/// channel and the new embeds, so everything but the ids is optional.
#[derive(Clone, Debug, Deserialize)]
pub struct PartialMessage {
    pub id: Snowflake,
    pub channel_id: Snowflake,
    #[serde(default)]
    pub guild_id: Option<Snowflake>,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub edited_timestamp: Option<DateTime<Utc>>,
    #[serde(default)]
    pub mentions: Option<Vec<User>>,
    #[serde(default)]
    pub attachments: Option<Vec<Attachment>>,
    #[serde(default)]
    pub embeds: Option<Vec<Embed>>,
    #[serde(default)]
    pub pinned: Option<bool>,
}
