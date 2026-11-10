use oxicord_model::{
    Activity, Emoji, Message, PartialMessage, Reaction, ReadState, Relationship, Snowflake, Status,
    gateway::{
        DispatchEvent, GuildDelete, GuildMemberUpdate, GuildRoleDelete, GuildRoleEvent,
        MemberListUpdate, MessageAck, MessageDelete, MessageDeleteBulk, PresenceUpdate,
        ReactionEvent, Ready, ReadySupplemental, TypingStart,
    },
    MemberList,
};

use crate::{Changes, Store, Timeline};

/// How long a TYPING_START keeps the indicator alive.
const TYPING_SECONDS: u64 = 10;

impl Store {
    /// Folds one gateway event into the state and reports what changed.
    pub fn apply(&mut self, event: DispatchEvent) -> Changes {
        let mut changes = Changes::default();

        match event {
            DispatchEvent::Ready(ready) => {
                self.on_ready(*ready);
                return Changes::full();
            }
            DispatchEvent::ReadySupplemental(s) => self.on_ready_supplemental(*s, &mut changes),
            DispatchEvent::MessageCreate(m) => self.on_message_create(*m, &mut changes),
            DispatchEvent::MessageUpdate(p) => self.on_message_update(*p, &mut changes),
            DispatchEvent::MessageDelete(d) => self.on_message_delete(d, &mut changes),
            DispatchEvent::MessageDeleteBulk(d) => self.on_message_delete_bulk(d, &mut changes),
            DispatchEvent::MessageAck(a) => self.on_message_ack(a, &mut changes),
            DispatchEvent::ReactionAdd(e) => self.on_reaction(&e, true, &mut changes),
            DispatchEvent::ReactionRemove(e) => self.on_reaction(&e, false, &mut changes),
            DispatchEvent::ReactionBatch(b) => self.on_reaction_batch(b, &mut changes),
            DispatchEvent::MemberChunk(chunk) => {
                self.add_users(chunk.members.into_iter().map(|m| m.user).collect());
                // Names show in message text, so any open list may change.
                return Changes::full();
            }
            DispatchEvent::VoiceChannelStatus(s) => {
                if let Some(channel) = self.channels.get_mut(&s.id) {
                    channel.status = s.status.filter(|text| !text.is_empty());
                    let guild = channel.guild_id;
                    self.mark_listing(guild, &mut changes);
                }
            }
            DispatchEvent::GuildCreate(g) => {
                let id = g.id;
                self.ingest_guild(*g, Vec::new());
                changes.guild_list = true;
                changes.channel_lists.insert(id);
            }
            DispatchEvent::VoiceStateUpdate(state) => {
                if let Some(guild) = state.guild_id {
                    changes.channel_lists.insert(guild);
                }
                self.put_voice(*state);
            }
            DispatchEvent::GuildUpdate(g) => self.on_guild_update(*g, &mut changes),
            DispatchEvent::GuildDelete(d) => self.on_guild_delete(d, &mut changes),
            DispatchEvent::GuildMemberUpdate(u) => self.on_member_update(u, &mut changes),
            DispatchEvent::GuildRoleCreate(e) | DispatchEvent::GuildRoleUpdate(e) => {
                self.on_role_upsert(e, &mut changes);
            }
            DispatchEvent::GuildRoleDelete(d) => self.on_role_delete(d, &mut changes),
            DispatchEvent::ChannelCreate(c) | DispatchEvent::ChannelUpdate(c) => {
                let mut channel = *c;
                // Updates don't always repeat the last message id.
                if channel.last_message_id.is_none() {
                    channel.last_message_id =
                        self.channels.get(&channel.id).and_then(|old| old.last_message_id);
                }
                // Thread updates leave out our own membership.
                if channel.member.is_none() && let Some(old) = self.channels.get(&channel.id) {
                    channel.member = old.member.clone();
                }
                // The voice status has its own event and isn't repeated here.
                if channel.status.is_none() && let Some(old) = self.channels.get(&channel.id) {
                    channel.status = old.status.clone();
                }
                let id = channel.id;
                let guild = channel.guild_id;
                self.ingest_channel(channel);
                self.mark_listing(guild, &mut changes);
                changes.messages.insert(id);
            }
            DispatchEvent::ChannelDelete(c) => {
                self.remove_channel(c.id);
                self.mark_listing(c.guild_id, &mut changes);
                changes.messages.insert(c.id);
            }
            DispatchEvent::GuildMemberListUpdate(update) => {
                self.on_member_list(*update, &mut changes);
            }
            DispatchEvent::PresenceUpdate(p) => self.on_presence(p, &mut changes),
            DispatchEvent::RelationshipAdd(r) => self.on_relationship_add(*r, &mut changes),
            DispatchEvent::PollVoteAdd(v) => self.on_poll_vote(v, true, &mut changes),
            DispatchEvent::PollVoteRemove(v) => self.on_poll_vote(v, false, &mut changes),
            DispatchEvent::ReactionsCleared(e) => self.on_reactions_cleared(e, &mut changes),
            DispatchEvent::PassiveUpdate(p) => self.on_passive_update(p, &mut changes),
            DispatchEvent::CurrentUserUpdate(user) => {
                self.users.insert(user.id, (*user).clone());
                self.me = Some(*user);
                changes.dm_list = true;
                changes.guild_list = true;
            }
            DispatchEvent::RecipientAdd(e) => self.on_recipient(e, true, &mut changes),
            DispatchEvent::RecipientRemove(e) => self.on_recipient(e, false, &mut changes),
            DispatchEvent::GuildEmojisUpdate(e) => {
                if let Some(guild) = self.guilds.get_mut(&e.guild_id) {
                    guild.emojis = e.emojis;
                }
            }
            DispatchEvent::GuildStickersUpdate(e) => {
                if let Some(guild) = self.guilds.get_mut(&e.guild_id) {
                    guild.stickers = e.stickers;
                }
            }
            DispatchEvent::ThreadListSync(sync) => self.on_thread_list_sync(sync, &mut changes),
            DispatchEvent::ThreadMembersUpdate(u) => self.on_thread_members(u, false, &mut changes),
            DispatchEvent::ThreadMemberUpdate(u) => self.on_thread_members(u, true, &mut changes),
            DispatchEvent::GuildSettingsUpdate(settings) => {
                let guild = settings.guild_id;
                self.set_guild_settings(*settings);
                changes.guild_list = true;
                changes.dm_list = true;
                if let Some(guild) = guild {
                    changes.channel_lists.insert(guild);
                }
            }
            DispatchEvent::RelationshipRemove(r) => {
                self.relationships.remove(&r.id);
                changes.relationships = true;
            }
            DispatchEvent::TypingStart(t) => self.on_typing(t, &mut changes),
            // The voice server is for the call itself, the store only tracks who is in it.
            DispatchEvent::Resumed
            | DispatchEvent::VoiceServerUpdate(_)
            | DispatchEvent::InteractionCreate(_)
            | DispatchEvent::InteractionSuccess(_)
            | DispatchEvent::InteractionFailure(_)
            | DispatchEvent::InteractionModalCreate(_)
            | DispatchEvent::AutocompleteResponse(_)
            | DispatchEvent::Unknown { .. } => {}
        }
        changes
    }

    /// Creates an empty timeline so live messages that arrive while history
    /// is still loading are kept instead of dropped.
    pub fn open_channel(&mut self, channel: Snowflake) {
        self.timelines.entry(channel).or_default();
    }

    /// Merges a page of history as returned by the REST API (newest first).
    /// `requested` is the limit that was asked for, a shorter page means the
    /// start of the channel was reached.
    pub fn set_history(
        &mut self,
        channel: Snowflake,
        mut page: Vec<Message>,
        requested: usize,
    ) -> Changes {
        let received = page.len();
        for message in &page {
            self.users.insert(message.author.id, message.author.clone());
        }
        page.sort_by_key(|m| m.id);

        let timeline = self.timelines.entry(channel).or_default();
        for message in page {
            timeline.insert(message);
        }
        timeline.set_has_more_before(received >= requested);

        let mut changes = Changes::default();
        changes.messages.insert(channel);
        changes
    }

    /// Shows a message as pinned or unpinned ahead of the server's echo.
    pub fn set_pinned(&mut self, channel: Snowflake, message: Snowflake, pinned: bool) -> Changes {
        let mut changes = Changes::default();
        if let Some(m) = self.timelines.get_mut(&channel).and_then(|t| t.get_mut(message)) {
            m.pinned = pinned;
            changes.messages.insert(channel);
        }
        changes
    }

    /// Replaces a channel's messages with a window around a message we are
    /// jumping to. Older history stays available to page in.
    pub fn show_window(&mut self, channel: Snowflake, page: Vec<Message>) -> Changes {
        for message in &page {
            self.users.insert(message.author.id, message.author.clone());
        }
        let mut timeline = Timeline::default();
        for message in page {
            timeline.insert(message);
        }
        timeline.set_has_more_before(true);
        timeline.set_detached(true);
        self.timelines.insert(channel, timeline);

        let mut changes = Changes::default();
        changes.messages.insert(channel);
        changes
    }

    /// Leaves a jumped-to window so the newest messages can be loaded again.
    pub fn return_to_present(&mut self, channel: Snowflake) -> Changes {
        self.timelines.insert(channel, Timeline::default());
        let mut changes = Changes::default();
        changes.messages.insert(channel);
        changes
    }

    /// Marks a channel read up to `message` ahead of the server's echo, so
    /// the unread badge clears without waiting a round trip.
    pub fn mark_read(&mut self, channel: Snowflake, message: Snowflake) -> Changes {
        let state = self.read_state_entry(channel);
        state.last_message_id = Some(message);
        state.mention_count = 0;

        let mut changes = Changes::default();
        self.touch_channel(channel, &mut changes);
        changes
    }

    fn on_ready(&mut self, ready: Ready) {
        *self = Self::default();

        for user in ready.users {
            self.users.insert(user.id, user);
        }
        self.users.insert(ready.user.id, ready.user.clone());
        self.session_id = Some(ready.session_id.clone());
        self.me = Some(ready.user);

        let mut members: Vec<_> = ready
            .merged_members
            .into_iter()
            .map(MemberList::into_vec)
            .collect();
        for (i, guild) in ready.guilds.into_iter().enumerate() {
            let own = members.get_mut(i).map(std::mem::take).unwrap_or_default();
            self.ingest_guild(guild, own);
        }

        for channel in ready.private_channels {
            self.ingest_channel(channel);
        }

        if let Some(list) = ready.user_guild_settings {
            for entry in list.entries {
                self.set_guild_settings(entry);
            }
        }

        for relationship in ready.relationships {
            self.insert_relationship(relationship);
        }

        if let Some(states) = ready.read_state {
            for state in states.entries() {
                self.read_states.insert(state.id, state.clone());
            }
        }
    }

    fn on_member_list(&mut self, update: MemberListUpdate, changes: &mut Changes) {
        let guild = update.guild_id;
        let state = self.member_lists.entry(guild).or_default();

        for member in state.apply(update) {
            if let Some(presence) = member.presence {
                self.set_presence(member.user.id, presence.status, presence.activities);
                changes.presences = true;
            }
            self.users.insert(member.user.id, member.user);
        }
        changes.members.insert(guild);
    }

    /// Presence is per user, so one entry serves every place they appear.
    fn set_presence(&mut self, user: Snowflake, status: Status, activities: Vec<Activity>) {
        if status == Status::Offline {
            self.presences.remove(&user);
        } else {
            self.presences.insert(user, status);
        }
        // Someone who went offline or invisible is doing nothing we may see.
        if status == Status::Offline || activities.is_empty() {
            self.activities.remove(&user);
        } else {
            self.activities.insert(user, activities);
        }
    }

    /// Picks our own roles out of the extra members READY left for later,
    /// and the presences of people we talk to. Roles decide which channels
    /// we can see, so the lists need a redraw.
    fn on_ready_supplemental(&mut self, extra: ReadySupplemental, changes: &mut Changes) {
        let me = self.my_id();

        if let Some(presences) = &extra.merged_presences {
            for entry in &presences.friends {
                self.set_presence(entry.user_id, entry.status, entry.activities.clone());
            }
            changes.presences = true;
            changes.dm_list = true;
        }

        for (guild, members) in extra.guilds.iter().zip(extra.merged_members) {
            let mine = members
                .into_vec()
                .into_iter()
                .find(|m| m.id().is_some() && m.id() == me);
            if let Some(member) = mine {
                self.my_roles.insert(guild.id, member.roles);
                changes.channel_lists.insert(guild.id);
                changes.guild_list = true;
            }
        }
    }

    fn on_message_create(&mut self, message: Message, changes: &mut Changes) {
        let channel_id = message.channel_id;
        self.users.insert(message.author.id, message.author.clone());
        for user in &message.mentions {
            self.users.entry(user.id).or_insert_with(|| user.clone());
        }

        let prior_last = self
            .channels
            .get(&channel_id)
            .and_then(|c| c.last_message_id);
        if let Some(channel) = self.channels.get_mut(&channel_id)
            && channel.last_message_id < Some(message.id)
        {
            channel.last_message_id = Some(message.id);
        }

        if let Some(typers) = self.typing.get_mut(&channel_id)
            && typers.remove(&message.author.id).is_some()
        {
            changes.typing.insert(channel_id);
        }

        let mine = Some(message.author.id) == self.my_id();
        let mentioned = !mine && self.mentions_me(&message);

        // A channel with no read state yet still needs a baseline, otherwise
        // its first unread message would never show as unread.
        let state = self.read_states.entry(channel_id).or_insert_with(|| ReadState {
            id: channel_id,
            last_message_id: Some(prior_last.unwrap_or_default()),
            mention_count: 0,
        });
        if mine {
            // Sending a message reads the channel on the server too.
            state.last_message_id = Some(message.id);
            state.mention_count = 0;
        } else if mentioned {
            state.mention_count += 1;
        }

        self.touch_channel(channel_id, changes);

        if let Some(timeline) = self.timelines.get_mut(&channel_id)
            && !timeline.is_detached()
        {
            timeline.insert_live(message);
            changes.messages.insert(channel_id);
        }
    }

    fn on_message_update(&mut self, update: PartialMessage, changes: &mut Changes) {
        let Some(message) = self
            .timelines
            .get_mut(&update.channel_id)
            .and_then(|t| t.get_mut(update.id))
        else {
            return;
        };

        if let Some(content) = update.content {
            message.content = content;
        }
        if update.edited_timestamp.is_some() {
            message.edited_timestamp = update.edited_timestamp;
        }
        if let Some(embeds) = update.embeds {
            message.embeds = embeds;
        }
        if let Some(attachments) = update.attachments {
            message.attachments = attachments;
        }
        if let Some(mentions) = update.mentions {
            message.mentions = mentions;
        }
        if let Some(pinned) = update.pinned {
            message.pinned = pinned;
        }
        if let Some(components) = update.components {
            message.components = components;
        }
        changes.messages.insert(update.channel_id);
    }

    fn on_message_delete(&mut self, delete: MessageDelete, changes: &mut Changes) {
        if let Some(timeline) = self.timelines.get_mut(&delete.channel_id)
            && timeline.remove(delete.id)
        {
            changes.messages.insert(delete.channel_id);
        }
    }

    fn on_message_delete_bulk(&mut self, delete: MessageDeleteBulk, changes: &mut Changes) {
        let Some(timeline) = self.timelines.get_mut(&delete.channel_id) else {
            return;
        };
        let mut removed = false;
        for id in delete.ids {
            removed |= timeline.remove(id);
        }
        if removed {
            changes.messages.insert(delete.channel_id);
        }
    }

    fn on_message_ack(&mut self, ack: MessageAck, changes: &mut Changes) {
        let state = self.read_state_entry(ack.channel_id);
        state.last_message_id = Some(ack.message_id);
        state.mention_count = ack.mention_count.unwrap_or(0);
        self.touch_channel(ack.channel_id, changes);
    }

    fn on_reaction(&mut self, event: &ReactionEvent, add: bool, changes: &mut Changes) {
        let mine = Some(event.user_id) == self.my_id();
        let Some(message) = self
            .timelines
            .get_mut(&event.channel_id)
            .and_then(|t| t.get_mut(event.message_id))
        else {
            return;
        };

        let found = message
            .reactions
            .iter()
            .position(|r| same_emoji(&r.emoji, &event.emoji));

        match (found, add) {
            (Some(i), true) => {
                message.reactions[i].count += 1;
                message.reactions[i].me |= mine;
            }
            (None, true) => message.reactions.push(Reaction {
                count: 1,
                me: mine,
                emoji: event.emoji.clone(),
            }),
            (Some(i), false) => {
                let reaction = &mut message.reactions[i];
                reaction.count = reaction.count.saturating_sub(1);
                if mine {
                    reaction.me = false;
                }
                if reaction.count == 0 {
                    message.reactions.remove(i);
                }
            }
            (None, false) => return,
        }
        changes.messages.insert(event.channel_id);
    }

    fn on_reaction_batch(&mut self, batch: oxicord_model::gateway::ReactionBatch, changes: &mut Changes) {
        let me = self.my_id();
        let Some(message) = self
            .timelines
            .get_mut(&batch.channel_id)
            .and_then(|t| t.get_mut(batch.message_id))
        else {
            return;
        };

        for entry in batch.reactions {
            if entry.users.is_empty() {
                continue;
            }
            let count = entry.users.len() as u32;
            let mine = me.is_some_and(|me| entry.users.contains(&me));
            match message
                .reactions
                .iter_mut()
                .find(|r| same_emoji(&r.emoji, &entry.emoji))
            {
                Some(reaction) => {
                    reaction.count += count;
                    reaction.me |= mine;
                }
                None => message.reactions.push(Reaction {
                    count,
                    me: mine,
                    emoji: entry.emoji,
                }),
            }
        }
        changes.messages.insert(batch.channel_id);
    }

    fn on_poll_vote(&mut self, vote: oxicord_model::gateway::PollVote, add: bool, changes: &mut Changes) {
        let mine = Some(vote.user_id) == self.my_id();
        let Some(poll) = self
            .timelines
            .get_mut(&vote.channel_id)
            .and_then(|t| t.get_mut(vote.message_id))
            .and_then(|m| m.poll.as_mut())
        else {
            return;
        };
        poll.apply_vote(vote.answer_id, add, mine);
        changes.messages.insert(vote.channel_id);
    }

    /// Records our own votes at once, ahead of the server.
    pub fn set_my_poll_votes(&mut self, channel: Snowflake, message: Snowflake, votes: &[u32]) -> Changes {
        let mut changes = Changes::default();
        if let Some(poll) = self
            .timelines
            .get_mut(&channel)
            .and_then(|t| t.get_mut(message))
            .and_then(|m| m.poll.as_mut())
        {
            poll.set_my_votes(votes);
            changes.messages.insert(channel);
        }
        changes
    }

    fn on_reactions_cleared(&mut self, event: oxicord_model::gateway::ReactionsCleared, changes: &mut Changes) {
        let Some(message) = self
            .timelines
            .get_mut(&event.channel_id)
            .and_then(|t| t.get_mut(event.message_id))
        else {
            return;
        };
        match &event.emoji {
            Some(emoji) => message.reactions.retain(|r| !same_emoji(&r.emoji, emoji)),
            None => message.reactions.clear(),
        }
        changes.messages.insert(event.channel_id);
    }

    /// Moves channels' last message forward so unread markers appear for
    /// servers too big to send every message to us.
    fn on_passive_update(&mut self, update: oxicord_model::gateway::PassiveUpdate, changes: &mut Changes) {
        for activity in update.updated_channels {
            let Some(channel) = self.channels.get_mut(&activity.id) else { continue };
            if activity.last_message_id > channel.last_message_id {
                channel.last_message_id = activity.last_message_id;
                changes.channel_lists.insert(update.guild_id);
                changes.guild_list = true;
            }
        }
    }

    fn on_recipient(&mut self, event: oxicord_model::gateway::RecipientChange, add: bool, changes: &mut Changes) {
        let Some(channel) = self.channels.get_mut(&event.channel_id) else { return };
        if add {
            if !channel.recipients.iter().any(|u| u.id == event.user.id) {
                self.users.insert(event.user.id, event.user.clone());
                channel.recipients.push(event.user);
            }
        } else {
            channel.recipients.retain(|u| u.id != event.user.id);
        }
        changes.dm_list = true;
    }

    fn on_guild_update(&mut self, update: oxicord_model::Guild, changes: &mut Changes) {
        let Some(guild) = self.guilds.get_mut(&update.id) else {
            return;
        };
        // An update without a name is a partial payload, so leave the
        // descriptive fields alone. A missing icon is legitimate, a missing
        // name is not.
        if !update.name.is_empty() {
            guild.name = update.name;
            guild.icon = update.icon;
        }
        guild.owner_id = update.owner_id.or(guild.owner_id);
        if !update.roles.is_empty() {
            guild.roles = update.roles;
        }
        changes.guild_list = true;
        changes.channel_lists.insert(update.id);
    }

    fn on_guild_delete(&mut self, delete: GuildDelete, changes: &mut Changes) {
        changes.guild_list = true;

        if delete.unavailable == Some(true) {
            if let Some(guild) = self.guilds.get_mut(&delete.id) {
                guild.unavailable = true;
            }
            return;
        }

        // Threads aren't in the per guild index, so scan for everything.
        let doomed: Vec<_> = self
            .channels
            .values()
            .filter(|c| c.guild_id == Some(delete.id))
            .map(|c| c.id)
            .collect();
        for id in doomed {
            self.remove_channel(id);
        }
        self.guild_channels.remove(&delete.id);
        self.voice.retain(|channel, _| self.channels.contains_key(channel));
        self.guilds.remove(&delete.id);
        self.my_roles.remove(&delete.id);
        self.pending.remove(&delete.id);
        self.member_lists.remove(&delete.id);
        self.guild_order.retain(|id| *id != delete.id);
        changes.channel_lists.insert(delete.id);
    }

    fn on_member_update(&mut self, update: GuildMemberUpdate, changes: &mut Changes) {
        if Some(update.user.id) != self.my_id() {
            return;
        }
        self.my_roles.insert(update.guild_id, update.roles);
        if let Some(pending) = update.pending {
            if pending {
                self.pending.insert(update.guild_id);
            } else {
                self.pending.remove(&update.guild_id);
            }
        }
        // Our roles decide which channels we see.
        changes.guild_list = true;
        changes.channel_lists.insert(update.guild_id);
    }

    fn on_role_upsert(&mut self, event: GuildRoleEvent, changes: &mut Changes) {
        let Some(guild) = self.guilds.get_mut(&event.guild_id) else {
            return;
        };
        match guild.roles.iter_mut().find(|r| r.id == event.role.id) {
            Some(existing) => *existing = event.role,
            None => guild.roles.push(event.role),
        }
        changes.guild_list = true;
        changes.channel_lists.insert(event.guild_id);
    }

    fn on_role_delete(&mut self, delete: GuildRoleDelete, changes: &mut Changes) {
        if let Some(guild) = self.guilds.get_mut(&delete.guild_id) {
            guild.roles.retain(|r| r.id != delete.role_id);
        }
        if let Some(roles) = self.my_roles.get_mut(&delete.guild_id) {
            roles.retain(|r| *r != delete.role_id);
        }
        changes.guild_list = true;
        changes.channel_lists.insert(delete.guild_id);
    }

    fn on_relationship_add(&mut self, relationship: Relationship, changes: &mut Changes) {
        self.insert_relationship(relationship);
        changes.relationships = true;
        // Names in the DM list can come from the same user.
        changes.dm_list = true;
    }

    /// READY lists the people in its own `users` and may omit them here, so
    /// keep whichever copy we get.
    fn insert_relationship(&mut self, mut relationship: Relationship) {
        if let Some(user) = relationship.user.take() {
            self.users.insert(user.id, user);
        }
        self.relationships.insert(relationship.id, relationship);
    }

    fn on_presence(&mut self, update: PresenceUpdate, changes: &mut Changes) {
        self.set_presence(update.user.id, update.status, update.activities);
        changes.presences = true;
    }

    fn on_typing(&mut self, typing: TypingStart, changes: &mut Changes) {
        if Some(typing.user_id) == self.my_id() {
            return;
        }
        self.typing
            .entry(typing.channel_id)
            .or_default()
            .insert(typing.user_id, typing.timestamp + TYPING_SECONDS);
        changes.typing.insert(typing.channel_id);
    }

    pub(crate) fn mentions_me(&self, message: &Message) -> bool {
        let Some(me) = self.my_id() else {
            return false;
        };
        let guild = message
            .guild_id
            .or_else(|| self.channels.get(&message.channel_id)?.guild_id);

        match guild {
            // Every message from someone else in a DM counts, as it does
            // in the official client.
            None => true,
            Some(guild) => {
                let roles = self.my_roles(guild);
                let (no_everyone, no_roles) = self.suppresses(guild);
                (message.mention_everyone && !no_everyone)
                    || message.mentions.iter().any(|u| u.id == me)
                    || (!no_roles && message.mention_roles.iter().any(|r| roles.contains(r)))
            }
        }
    }

    fn read_state_entry(&mut self, channel: Snowflake) -> &mut ReadState {
        self.read_states.entry(channel).or_insert_with(|| ReadState {
            id: channel,
            last_message_id: None,
            mention_count: 0,
        })
    }

    /// Flags the list that shows `channel`'s unread state.
    fn touch_channel(&self, channel: Snowflake, changes: &mut Changes) {
        let guild = self.channels.get(&channel).and_then(|c| c.guild_id);
        self.mark_listing(guild, changes);
    }

    fn mark_listing(&self, guild: Option<Snowflake>, changes: &mut Changes) {
        match guild {
            Some(guild) => {
                changes.channel_lists.insert(guild);
                changes.guild_list = true;
            }
            None => changes.dm_list = true,
        }
    }
}

/// Custom emoji match by id. Unicode ones by name. The `animated` flag is
/// not always present on both sides, so it can't take part.
fn same_emoji(a: &Emoji, b: &Emoji) -> bool {
    match (a.id, b.id) {
        (Some(x), Some(y)) => x == y,
        (None, None) => a.name == b.name,
        _ => false,
    }
}
