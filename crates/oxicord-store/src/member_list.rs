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

/// The rows of one list, which the server edits in place. We only ever hold
/// the part it has sent us, so some slots are empty.
#[derive(Debug, Default)]
struct Slots {
    slots: Vec<Option<Entry>>,
    /// How many members sit under each group. The server sends these beside
    /// the slots, and not on the header slots themselves.
    group_counts: HashMap<String, u64>,
}

/// A guild's member lists as the server describes them. Channels that see
/// the same people share a list, others have their own, and the server may
/// keep sending edits for one we no longer look at. Each list keeps its own
/// rows, and the one on screen is the one the server last synced for us.
#[derive(Debug, Default)]
pub struct MemberListState {
    current: String,
    lists: HashMap<String, Slots>,
    member_count: Option<u64>,
}

impl MemberListState {
    fn shown(&self) -> Option<&Slots> {
        self.lists.get(&self.current)
    }

    pub fn entries(&self) -> &[Option<Entry>] {
        self.shown().map_or(&[], |list| list.slots.as_slice())
    }

    pub fn member_count(&self) -> Option<u64> {
        self.member_count
    }

    /// The size of a group, preferring the figure the server sent on its own
    /// over the one carried by the header slot.
    pub fn group_count(&self, group: &str, on_slot: u64) -> u64 {
        self.shown().and_then(|list| list.group_counts.get(group).copied()).unwrap_or(on_slot)
    }

    /// Applies one update. Returns the members it mentioned, so the caller
    /// can learn their user details and presence.
    pub(crate) fn apply(&mut self, update: MemberListUpdate) -> Vec<ListMember> {
        if update.member_count.is_some() {
            self.member_count = update.member_count;
        }
        let id = update.id;
        let mut seen = Vec::new();
        let mut drop_list = false;

        let list = self.lists.entry(id.clone()).or_default();
        for group in &update.groups {
            list.group_counts.insert(group.id.clone(), group.count);
        }

        for op in update.ops {
            match op {
                // Asking for a range is what shows a list, so a sync is what
                // makes this one the list on screen.
                ListOp::Sync { range, items } => {
                    list.sync(range[0], items, &mut seen);
                    self.current.clone_from(&id);
                }
                ListOp::Insert { index, item } => {
                    if index <= MAX_SLOTS {
                        list.slots.resize(list.slots.len().max(index), None);
                        list.slots.insert(index, Some(entry(item, &mut seen)));
                    }
                }
                ListOp::Update { index, item } => {
                    if index < MAX_SLOTS {
                        list.slots.resize(list.slots.len().max(index + 1), None);
                        list.slots[index] = Some(entry(item, &mut seen));
                    }
                }
                ListOp::Delete { index } => {
                    if index < list.slots.len() {
                        list.slots.remove(index);
                    }
                }
                ListOp::Invalidate { range } => {
                    let end = range[1].min(list.slots.len().saturating_sub(1));
                    for slot in list.slots.iter_mut().take(end + 1).skip(range[0]) {
                        *slot = None;
                    }
                    // A list we aren't looking at will not be asked for again.
                    drop_list = id != self.current;
                }
            }
        }
        if drop_list {
            self.lists.remove(&id);
        }
        seen
    }
}

impl Slots {
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
