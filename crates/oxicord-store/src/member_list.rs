use std::collections::HashMap;

use oxicord_model::{
    Snowflake,
    gateway::{ListItem, ListMember, ListOp, MemberListUpdate},
};

/// Guards against a hostile or corrupt index making us allocate gigabytes.
const MAX_SLOTS: usize = 100_000;

/// One row of the member list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Entry {
    /// A header such as "online", "offline" or a role id, with how many
    /// members sit under it.
    Group { id: String, count: u64 },
    Member {
        user: Snowflake,
        nick: Option<String>,
        roles: Vec<Snowflake>,
    },
}

/// A guild's member list as the server describes it: a run of slots that
/// the server edits in place. We only ever hold the part it has sent us, so
/// some slots are empty.
#[derive(Debug, Default)]
pub struct MemberListState {
    list_id: String,
    slots: Vec<Option<Entry>>,
    member_count: Option<u64>,
    /// How many members sit under each group. The server sends these beside
    /// the slots, and not on the header slots themselves.
    group_counts: HashMap<String, u64>,
}

impl MemberListState {
    pub fn entries(&self) -> &[Option<Entry>] {
        &self.slots
    }

    pub fn member_count(&self) -> Option<u64> {
        self.member_count
    }

    /// The size of a group, preferring the figure the server sent on its own
    /// over the one carried by the header slot.
    pub fn group_count(&self, group: &str, on_slot: u64) -> u64 {
        self.group_counts.get(group).copied().unwrap_or(on_slot)
    }

    /// Applies one update. Returns the members it mentioned, so the caller
    /// can learn their user details and presence.
    pub(crate) fn apply(&mut self, update: MemberListUpdate) -> Vec<ListMember> {
        // A different channel can have a different list. Its slots mean
        // nothing against the old ones, so start over.
        if update.id != self.list_id {
            self.list_id = update.id;
            self.slots.clear();
            self.group_counts.clear();
        }
        if update.member_count.is_some() {
            self.member_count = update.member_count;
        }
        for group in &update.groups {
            self.group_counts.insert(group.id.clone(), group.count);
        }

        let mut seen = Vec::new();
        for op in update.ops {
            match op {
                ListOp::Sync { range, items } => self.sync(range[0], items, &mut seen),
                ListOp::Insert { index, item } => {
                    if index <= MAX_SLOTS {
                        self.slots.resize(self.slots.len().max(index), None);
                        self.slots.insert(index, Some(entry(item, &mut seen)));
                    }
                }
                ListOp::Update { index, item } => {
                    if index < MAX_SLOTS {
                        self.slots.resize(self.slots.len().max(index + 1), None);
                        self.slots[index] = Some(entry(item, &mut seen));
                    }
                }
                ListOp::Delete { index } => {
                    if index < self.slots.len() {
                        self.slots.remove(index);
                    }
                }
                ListOp::Invalidate { range } => {
                    let end = range[1].min(self.slots.len().saturating_sub(1));
                    for slot in self.slots.iter_mut().take(end + 1).skip(range[0]) {
                        *slot = None;
                    }
                }
            }
        }
        seen
    }

    fn sync(&mut self, start: usize, items: Vec<ListItem>, seen: &mut Vec<ListMember>) {
        let end = start + items.len();
        if end > MAX_SLOTS {
            return;
        }
        if self.slots.len() < end {
            self.slots.resize(end, None);
        }
        for (offset, item) in items.into_iter().enumerate() {
            self.slots[start + offset] = Some(entry(item, seen));
        }
    }
}

fn entry(item: ListItem, seen: &mut Vec<ListMember>) -> Entry {
    match item {
        ListItem::Group { group } => Entry::Group {
            id: group.id,
            count: group.count,
        },
        ListItem::Member { member } => {
            let entry = Entry::Member {
                user: member.user.id,
                nick: member.nick.clone(),
                roles: member.roles.clone(),
            };
            seen.push(member);
            entry
        }
    }
}
