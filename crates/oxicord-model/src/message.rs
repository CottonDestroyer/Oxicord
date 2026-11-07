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
    /// Buttons, menus and layout a bot attached.
    #[serde(default, deserialize_with = "crate::component::components")]
    pub components: Vec<crate::Component>,
    #[serde(default, deserialize_with = "crate::poll::poll")]
    pub poll: Option<crate::Poll>,
    #[serde(default)]
    pub sticker_items: Vec<StickerItem>,
    /// Present when this message forwards another.
    #[serde(default)]
    pub message_snapshots: Vec<MessageSnapshot>,
    /// The app that sent an interaction response or webhook message.
    #[serde(default)]
    pub application_id: Option<Snowflake>,
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
    #[serde(default, deserialize_with = "crate::component::optional_components")]
    pub components: Option<Vec<crate::Component>>,
}

/// A sticker sent with a message.
#[derive(Clone, Debug, Deserialize)]
pub struct StickerItem {
    pub id: Snowflake,
    #[serde(default)]
    pub name: String,
    /// 1 PNG, 2 animated PNG, 3 Lottie, 4 GIF.
    #[serde(default)]
    pub format_type: u8,
    /// False for a server sticker past the slots its boost level pays for.
    /// Only server stickers carry it.
    #[serde(default = "available")]
    pub available: bool,
}

fn available() -> bool {
    true
}

impl StickerItem {
    /// Lottie stickers are vector animations that need their own player.
    pub fn is_lottie(&self) -> bool {
        self.format_type == 3
    }

    pub fn extension(&self) -> &'static str {
        if self.format_type == 4 { "gif" } else { "png" }
    }
}

/// The content of a message that was forwarded. It has no id or author of its
/// own, only what was copied.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct SnapshotMessage {
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub attachments: Vec<Attachment>,
    #[serde(default)]
    pub embeds: Vec<Embed>,
    #[serde(default)]
    pub mentions: Vec<User>,
    #[serde(default)]
    pub sticker_items: Vec<StickerItem>,
}

/// A message we saved for later, possibly with a reminder attached.
#[derive(Clone, Debug, Deserialize)]
pub struct SavedMessage {
    pub message: Message,
    #[serde(default)]
    pub save_data: SavedMessageData,
}

/// The bookkeeping Discord keeps alongside a saved message.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct SavedMessageData {
    #[serde(default)]
    pub due_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub notes: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct MessageSnapshot {
    pub message: SnapshotMessage,
}

#[cfg(test)]
mod extras_tests {
    use super::*;

    fn base() -> serde_json::Value {
        serde_json::json!({
            "id": "1", "channel_id": "2", "author": { "id": "3", "username": "a" },
            "timestamp": "2024-05-01T12:00:00.000000+00:00", "type": 0
        })
    }

    #[test]
    fn stickers_and_forwards_parse() {
        let mut json = base();
        json["sticker_items"] = serde_json::json!([{ "id": "9", "name": "wave", "format_type": 4 }, { "id": "8", "format_type": 3 }]);
        json["message_snapshots"] = serde_json::json!([{ "message": { "content": "hi", "attachments": [], "type": 0 } }]);
        let m: Message = serde_json::from_value(json).unwrap();
        assert_eq!(m.sticker_items[0].extension(), "gif");
        assert!(m.sticker_items[1].is_lottie());
        assert_eq!(m.message_snapshots[0].message.content, "hi");
    }

    #[test]
    fn plain_messages_have_neither() {
        let m: Message = serde_json::from_value(base()).unwrap();
        assert!(m.sticker_items.is_empty() && m.message_snapshots.is_empty());
    }
}
