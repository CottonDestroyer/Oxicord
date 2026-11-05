//! Threads and forum posts. A thread is a channel whose parent is another
//! channel, so most of the machinery is shared. This file holds what is
//! specific to them: which ones are open, whether we have joined, and the
//! membership events.

use oxicord_model::{
    Channel, ThreadMember,
    gateway::{ThreadListSync, ThreadMembersUpdate},
    Snowflake,
};

use crate::{Changes, Store};

impl Store {
    /// Threads under `parent` that are still open, newest activity first.
    pub fn open_threads(&self, parent: Snowflake) -> Vec<&Channel> {
        let Some(guild) = self.channels.get(&parent).and_then(|c| c.guild_id) else {
            return Vec::new();
        };
        let mut threads: Vec<_> = self
            .guild_threads
            .get(&guild)
            .into_iter()
            .flatten()
            .filter_map(|id| self.channels.get(id))
            .filter(|t| t.parent_id == Some(parent) && !t.archived())
            .collect();
        threads.sort_by(|a, b| b.last_message_id.cmp(&a.last_message_id).then(b.id.cmp(&a.id)));
        threads
    }

    /// Threads we have joined, which are the ones the channel list shows.
    pub fn joined_threads(&self, parent: Snowflake) -> Vec<&Channel> {
        self.open_threads(parent).into_iter().filter(|t| t.joined()).collect()
    }

    /// The tags a forum offers, as (id, name).
    pub fn forum_tags(&self, forum: Snowflake) -> Vec<(Snowflake, String)> {
        self.channels
            .get(&forum)
            .map(|c| c.available_tags.iter().map(|t| (t.id, t.name.clone())).collect())
            .unwrap_or_default()
    }

    /// Takes in a thread we learned about outside the gateway, such as a
    /// search result.
    pub fn add_thread(&mut self, thread: Channel, joined_as_me: bool) {
        let mut thread = thread;
        if joined_as_me && thread.member.is_none() {
            thread.member = Some(ThreadMember::default());
        }
        if let Some(old) = self.channels.get(&thread.id) {
            if thread.member.is_none() {
                thread.member = old.member.clone();
            }
            if thread.last_message_id.is_none() {
                thread.last_message_id = old.last_message_id;
            }
        }
        self.ingest_channel(thread);
    }

    pub(crate) fn on_thread_list_sync(&mut self, sync: ThreadListSync, changes: &mut Changes) {
        let me = self.my_id();
        // The sync lists every open thread under its parents, so a thread
        // we hold under those parents that isn't listed has been archived or
        // deleted since.
        let covered = |parent: Option<Snowflake>| match (&sync.channel_ids, parent) {
            (None, _) => true,
            (Some(ids), Some(p)) => ids.contains(&p),
            (Some(_), None) => false,
        };
        let listed: Vec<Snowflake> = sync.threads.iter().map(|t| t.id).collect();
        let stale: Vec<Snowflake> = self
            .guild_threads
            .get(&sync.guild_id)
            .into_iter()
            .flatten()
            .filter_map(|id| self.channels.get(id))
            .filter(|t| covered(t.parent_id) && !listed.contains(&t.id))
            .map(|t| t.id)
            .collect();
        for id in stale {
            self.remove_channel(id);
        }

        for mut thread in sync.threads {
            thread.guild_id.get_or_insert(sync.guild_id);
            let mine = sync
                .members
                .iter()
                .any(|m| m.id == Some(thread.id) && m.user_id.is_none_or(|u| Some(u) == me));
            self.add_thread(thread, mine);
        }
        changes.channel_lists.insert(sync.guild_id);
        changes.guild_list = true;
    }

    /// THREAD_MEMBER_UPDATE (ours) and THREAD_MEMBERS_UPDATE (anyone's).
    pub(crate) fn on_thread_members(
        &mut self,
        update: ThreadMembersUpdate,
        ours_only: bool,
        changes: &mut Changes,
    ) {
        let me = self.my_id();
        let Some(thread) = self.channels.get_mut(&update.id) else { return };

        if let Some(count) = update.member_count {
            thread.member_count = Some(count);
        }
        if ours_only {
            thread.member = Some(ThreadMember { flags: update.flags });
        }
        if update.added_members.iter().any(|m| m.user_id.is_some() && m.user_id == me) {
            thread.member = Some(ThreadMember::default());
        }
        if me.is_some_and(|me| update.removed_member_ids.contains(&me)) {
            thread.member = None;
        }
        let guild = thread.guild_id;
        if let Some(guild) = guild {
            changes.channel_lists.insert(guild);
        }
        changes.messages.insert(update.id);
    }
}
