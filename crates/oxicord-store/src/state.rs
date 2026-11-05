use std::collections::HashMap;

use oxicord_model::{
    Channel, ChannelType, Guild, Member, Permissions, ReadState, Snowflake, Status, User,
};

use crate::{
    perms::{self, DM_PERMISSIONS},
    timeline::Timeline,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Unread {
    pub unread: bool,
    pub mentions: u32,
}

/// A run of channels under one category. `category` is `None` for channels
/// that sit above all categories.
#[derive(Debug)]
pub struct ChannelGroup<'a> {
    pub category: Option<&'a Channel>,
    pub channels: Vec<&'a Channel>,
}

#[derive(Default)]
pub struct Store {
    pub(crate) me: Option<User>,
    pub(crate) users: HashMap<Snowflake, User>,
    pub(crate) guild_order: Vec<Snowflake>,
    pub(crate) guilds: HashMap<Snowflake, Guild>,
    /// Our role ids per guild.
    pub(crate) my_roles: HashMap<Snowflake, Vec<Snowflake>>,
    pub(crate) channels: HashMap<Snowflake, Channel>,
    /// Channel ids per guild, so listing a guild doesn't scan every channel
    /// we know about. Threads are left out, they don't appear in the sidebar.
    pub(crate) guild_channels: HashMap<Snowflake, Vec<Snowflake>>,
    /// A channel only has an entry once someone opened it.
    pub(crate) timelines: HashMap<Snowflake, Timeline>,
    pub(crate) read_states: HashMap<Snowflake, ReadState>,
    pub(crate) presences: HashMap<Snowflake, Status>,
    /// Channel to user to the unix second the indicator expires.
    pub(crate) typing: HashMap<Snowflake, HashMap<Snowflake, u64>>,
}

impl Store {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn me(&self) -> Option<&User> {
        self.me.as_ref()
    }

    pub fn user(&self, id: Snowflake) -> Option<&User> {
        self.users.get(&id)
    }

    pub fn presence(&self, user: Snowflake) -> Status {
        self.presences.get(&user).copied().unwrap_or_default()
    }

    /// Guilds in the order the server listed them.
    pub fn guilds(&self) -> impl Iterator<Item = &Guild> {
        self.guild_order.iter().filter_map(|id| self.guilds.get(id))
    }

    pub fn guild(&self, id: Snowflake) -> Option<&Guild> {
        self.guilds.get(&id)
    }

    pub fn channel(&self, id: Snowflake) -> Option<&Channel> {
        self.channels.get(&id)
    }

    pub fn timeline(&self, channel: Snowflake) -> Option<&Timeline> {
        self.timelines.get(&channel)
    }

    pub fn my_roles(&self, guild: Snowflake) -> &[Snowflake] {
        self.my_roles.get(&guild).map_or(&[], Vec::as_slice)
    }

    /// What we may do in `channel`. Threads inherit from their parent.
    pub fn permissions_in(&self, channel: &Channel) -> Permissions {
        let Some(guild_id) = channel.guild_id else {
            return DM_PERMISSIONS;
        };
        let (Some(me), Some(guild)) = (self.me.as_ref(), self.guilds.get(&guild_id)) else {
            return Permissions::empty();
        };

        let roles = self.my_roles(guild_id);
        let base = perms::base_permissions(guild, me.id, roles);

        let source = if channel.kind.is_thread() {
            channel
                .parent_id
                .and_then(|p| self.channels.get(&p))
                .unwrap_or(channel)
        } else {
            channel
        };
        perms::channel_permissions(base, guild_id, me.id, roles, &source.permission_overwrites)
    }

    pub fn can_view(&self, channel: &Channel) -> bool {
        self.permissions_in(channel)
            .contains(Permissions::VIEW_CHANNEL)
    }

    /// The sidebar for a guild: viewable channels grouped under their
    /// categories, in display order. Categories with nothing visible in
    /// them are dropped.
    pub fn channel_groups(&self, guild: Snowflake) -> Vec<ChannelGroup<'_>> {
        let all = self
            .guild_channels
            .get(&guild)
            .into_iter()
            .flatten()
            .filter_map(|id| self.channels.get(id));

        let mut categories = Vec::new();
        let mut visible = Vec::new();
        for channel in all {
            match channel.kind {
                ChannelType::GuildCategory => categories.push(channel),
                ChannelType::Unknown => {}
                _ if self.can_view(channel) => visible.push(channel),
                _ => {}
            }
        }

        categories.sort_by_key(|c| (c.position, c.id));
        // Text channels come before voice ones within a category.
        visible.sort_by_key(|c| (is_voice(c.kind), c.position, c.id));

        let mut groups = Vec::new();

        let loose: Vec<_> = visible
            .iter()
            .copied()
            .filter(|c| !categories.iter().any(|cat| Some(cat.id) == c.parent_id))
            .collect();
        if !loose.is_empty() {
            groups.push(ChannelGroup {
                category: None,
                channels: loose,
            });
        }

        for category in categories {
            let channels: Vec<_> = visible
                .iter()
                .copied()
                .filter(|c| c.parent_id == Some(category.id))
                .collect();
            if !channels.is_empty() {
                groups.push(ChannelGroup {
                    category: Some(category),
                    channels,
                });
            }
        }
        groups
    }

    /// Open DMs and group DMs, most recently active first.
    pub fn dm_channels(&self) -> Vec<&Channel> {
        let mut dms: Vec<_> = self.channels.values().filter(|c| c.kind.is_dm()).collect();
        dms.sort_by(|a, b| {
            b.last_message_id
                .cmp(&a.last_message_id)
                .then_with(|| b.id.cmp(&a.id))
        });
        dms
    }

    /// What to show in the DM list for a channel.
    pub fn dm_name(&self, channel: &Channel) -> String {
        if let Some(name) = channel.name.as_deref().filter(|n| !n.is_empty()) {
            return name.to_owned();
        }
        let names: Vec<_> = channel.recipients.iter().map(User::display_name).collect();
        if names.is_empty() {
            "Unknown".to_owned()
        } else {
            names.join(", ")
        }
    }

    pub fn unread(&self, channel: Snowflake) -> Unread {
        let (Some(chan), Some(read)) = (self.channels.get(&channel), self.read_states.get(&channel))
        else {
            return Unread::default();
        };
        Unread {
            unread: chan.last_message_id > read.last_message_id,
            mentions: read.mention_count,
        }
    }

    /// Rolls channel state up for the guild rail. Channels we can't view
    /// don't count.
    pub fn guild_unread(&self, guild: Snowflake) -> Unread {
        let mut total = Unread::default();
        for id in self.guild_channels.get(&guild).into_iter().flatten() {
            let Some(channel) = self.channels.get(id) else {
                continue;
            };
            if channel.kind == ChannelType::GuildCategory || !self.can_view(channel) {
                continue;
            }
            let u = self.unread(*id);
            total.unread |= u.unread;
            total.mentions += u.mentions;
        }
        total
    }

    /// Users typing in `channel` right now, excluding ourselves.
    pub fn typing_users(&self, channel: Snowflake, now_unix: u64) -> Vec<&User> {
        let Some(typers) = self.typing.get(&channel) else {
            return Vec::new();
        };
        let mut live: Vec<_> = typers
            .iter()
            .filter(|(_, expires)| **expires > now_unix)
            .filter_map(|(user, _)| self.users.get(user))
            .collect();
        live.sort_by_key(|u| u.id);
        live
    }

    pub(crate) fn my_id(&self) -> Option<Snowflake> {
        self.me.as_ref().map(|u| u.id)
    }

    /// Adds a guild and its channels. `extra_members` covers payloads that
    /// deliver members separately from the guild, like READY.
    pub(crate) fn ingest_guild(&mut self, mut guild: Guild, extra_members: Vec<Member>) {
        let me = self.my_id();

        let mine = extra_members
            .iter()
            .chain(&guild.members)
            .find(|m| m.id().is_some() && m.id() == me);
        if let Some(member) = mine {
            self.my_roles.insert(guild.id, member.roles.clone());
        }

        let channels: Vec<_> = guild
            .channels
            .drain(..)
            .chain(guild.threads.drain(..))
            .collect();
        for channel in channels {
            self.ingest_channel(channel);
        }
        guild.members.clear();

        if !self.guild_order.contains(&guild.id) {
            self.guild_order.push(guild.id);
        }
        self.guilds.insert(guild.id, guild);
    }

    pub(crate) fn ingest_channel(&mut self, mut channel: Channel) {
        // READY sends private channels with ids only, to be joined against
        // the top level user list.
        if channel.recipients.is_empty() && !channel.recipient_ids.is_empty() {
            channel.recipients = channel
                .recipient_ids
                .iter()
                .filter_map(|id| self.users.get(id).cloned())
                .collect();
        }
        for user in &channel.recipients {
            self.users.insert(user.id, user.clone());
        }

        if let Some(guild) = channel.guild_id
            && !channel.kind.is_thread()
        {
            let ids = self.guild_channels.entry(guild).or_default();
            if !ids.contains(&channel.id) {
                ids.push(channel.id);
            }
        }
        self.channels.insert(channel.id, channel);
    }

    pub(crate) fn remove_channel(&mut self, id: Snowflake) -> Option<Channel> {
        let channel = self.channels.remove(&id)?;
        if let Some(ids) = channel
            .guild_id
            .and_then(|g| self.guild_channels.get_mut(&g))
        {
            ids.retain(|c| *c != id);
        }
        self.timelines.remove(&id);
        self.read_states.remove(&id);
        self.typing.remove(&id);
        Some(channel)
    }
}

fn is_voice(kind: ChannelType) -> bool {
    matches!(kind, ChannelType::GuildVoice | ChannelType::GuildStageVoice)
}
