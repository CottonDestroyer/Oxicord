//! Per-server and per-channel notification settings: what is muted and which
//! messages should interrupt us. The resolution order follows the official
//! client: a channel's own setting, then its category's, then the server's,
//! then the server's default.

use chrono::{DateTime, Utc};
use oxicord_model::{
    Channel, GuildNotificationSettings, Message, NotifyLevel, Snowflake, mute_active,
};

use crate::Store;

impl Store {
    pub(crate) fn set_guild_settings(&mut self, settings: GuildNotificationSettings) {
        self.notifications.insert(settings.guild_id, settings);
    }

    pub fn hide_muted_channels(&self, guild: Snowflake) -> bool {
        self.settings_for(Some(guild))
            .map(|s| s.hide_muted_channels)
            .unwrap_or(false)
    }

    fn settings_for(&self, guild: Option<Snowflake>) -> Option<&GuildNotificationSettings> {
        self.notifications.get(&guild)
    }

    pub fn guild_muted(&self, guild: Snowflake) -> bool {
        self.guild_muted_at(guild, Utc::now())
    }

    fn guild_muted_at(&self, guild: Snowflake, now: DateTime<Utc>) -> bool {
        self.settings_for(Some(guild))
            .is_some_and(|s| mute_active(s.muted, s.mute_config.as_ref(), now))
    }

    /// Whether `channel` is muted on its own, through its category, or
    /// through its server.
    pub fn is_muted(&self, channel: &Channel) -> bool {
        self.is_muted_at(channel, Utc::now())
    }

    pub(crate) fn is_muted_at(&self, channel: &Channel, now: DateTime<Utc>) -> bool {
        let own = |id: Snowflake| {
            self.settings_for(channel.guild_id)
                .and_then(|s| s.channel_overrides.iter().find(|o| o.channel_id == id))
                .is_some_and(|o| mute_active(o.muted, o.mute_config.as_ref(), now))
        };
        own(channel.id)
            || channel.parent_id.is_some_and(own)
            || channel.guild_id.is_some_and(|g| self.guild_muted_at(g, now))
    }

    /// Which messages in `channel` should notify us.
    pub fn notify_level(&self, channel: &Channel) -> NotifyLevel {
        let settings = self.settings_for(channel.guild_id);
        let overridden = |id: Snowflake| {
            settings
                .and_then(|s| s.channel_overrides.iter().find(|o| o.channel_id == id))
                .and_then(|o| NotifyLevel::from_wire(o.message_notifications))
        };

        overridden(channel.id)
            .or_else(|| channel.parent_id.and_then(overridden))
            .or_else(|| settings.and_then(|s| NotifyLevel::from_wire(s.message_notifications)))
            .unwrap_or_else(|| match channel.guild_id.and_then(|g| self.guilds.get(&g)) {
                Some(guild) if guild.default_message_notifications == 1 => NotifyLevel::Mentions,
                _ => NotifyLevel::All,
            })
    }

    /// Whether a message from someone else should raise a notification:
    /// not muted, and allowed by the level that applies to its channel.
    pub fn is_notable(&self, message: &Message) -> bool {
        if Some(message.author.id) == self.my_id() {
            return false;
        }
        let Some(channel) = self.channels.get(&message.channel_id) else {
            return self.mentions_me(message);
        };
        if self.is_muted(channel) {
            return false;
        }
        match self.notify_level(channel) {
            NotifyLevel::Nothing => false,
            NotifyLevel::All => true,
            NotifyLevel::Mentions => self.mentions_me(message),
        }
    }

    /// The level that applies to a server as a whole: its own choice, else
    /// the default it was created with.
    pub fn guild_notify_level(&self, guild: Snowflake) -> NotifyLevel {
        self.settings_for(Some(guild))
            .and_then(|s| NotifyLevel::from_wire(s.message_notifications))
            .unwrap_or_else(|| match self.guilds.get(&guild) {
                Some(g) if g.default_message_notifications == 1 => NotifyLevel::Mentions,
                _ => NotifyLevel::All,
            })
    }

    pub fn suppresses_everyone(&self, guild: Snowflake) -> bool {
        self.suppresses(guild).0
    }

    /// A channel's own notification choice, if it has one.
    pub fn channel_level_override(&self, channel: &Channel) -> Option<NotifyLevel> {
        self.settings_for(channel.guild_id)?
            .channel_overrides
            .iter()
            .find(|o| o.channel_id == channel.id)
            .and_then(|o| NotifyLevel::from_wire(o.message_notifications))
    }

    /// Channels of `guild` we can see with something unread, each paired with
    /// its newest message, ready to be acknowledged.
    pub fn unread_in_guild(&self, guild: Snowflake) -> Vec<(Snowflake, Snowflake)> {
        self.guild_channels
            .get(&guild)
            .into_iter()
            .flatten()
            .filter_map(|id| self.channels.get(id))
            .filter(|c| c.kind != oxicord_model::ChannelType::GuildCategory && self.can_view(c))
            .filter(|c| self.unread(c.id).unread || self.unread(c.id).mentions > 0)
            .filter_map(|c| Some((c.id, c.last_message_id?)))
            .collect()
    }

    /// Whether `@everyone` and role mentions count for this server.
    pub(crate) fn suppresses(&self, guild: Snowflake) -> (bool, bool) {
        self.settings_for(Some(guild))
            .map_or((false, false), |s| (s.suppress_everyone, s.suppress_roles))
    }
}
