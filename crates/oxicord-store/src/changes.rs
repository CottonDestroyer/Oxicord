use std::collections::HashSet;

use oxicord_model::Snowflake;

/// What an applied event touched, so the UI only redraws the parts that
/// could have changed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Changes {
    /// Everything. Set after READY, when there is nothing worth diffing.
    pub full: bool,
    /// The guild rail: membership, order, names, icons or unread pips.
    pub guild_list: bool,
    /// Guilds whose channel list changed.
    pub channel_lists: HashSet<Snowflake>,
    pub dm_list: bool,
    /// Channels whose message list changed.
    pub messages: HashSet<Snowflake>,
    /// Channels whose typing indicator changed.
    pub typing: HashSet<Snowflake>,
    pub presences: bool,
    /// The friends list: someone was added, accepted or removed.
    pub relationships: bool,
    /// Guilds whose member list changed.
    pub members: HashSet<Snowflake>,
}

impl Changes {
    pub fn full() -> Self {
        Self {
            full: true,
            ..Self::default()
        }
    }

    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    pub fn affects_channel_list(&self, guild: Snowflake) -> bool {
        self.full || self.channel_lists.contains(&guild)
    }

    pub fn affects_messages(&self, channel: Snowflake) -> bool {
        self.full || self.messages.contains(&channel)
    }

    /// Folds another batch in, for when the UI coalesces several events
    /// into one redraw.
    pub fn merge(&mut self, other: Changes) {
        self.full |= other.full;
        self.guild_list |= other.guild_list;
        self.dm_list |= other.dm_list;
        self.presences |= other.presences;
        self.relationships |= other.relationships;
        self.members.extend(other.members);
        self.channel_lists.extend(other.channel_lists);
        self.messages.extend(other.messages);
        self.typing.extend(other.typing);
    }
}
