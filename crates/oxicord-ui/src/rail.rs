//! How servers and folders are laid out in the guild rail.

use oxicord_model::Snowflake;
use oxicord_settings::GuildFolder;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Entry {
    Guild(Snowflake),
    Folder {
        /// The folder's id, which stays the same when it is renamed or reordered.
        key: u64,
        name: Option<String>,
        color: Option<u32>,
        guilds: Vec<Snowflake>,
    },
}

/// Lays out `known` (every server we are in) following the account's folder
/// arrangement. Servers the arrangement doesn't mention yet, usually ones
/// just joined, come first, as they do in the official client.
pub fn arrange(folders: &[GuildFolder], known: &[Snowflake]) -> Vec<Entry> {
    let mut placed: Vec<Snowflake> = Vec::new();
    let mut entries = Vec::new();

    for folder in folders {
        let guilds: Vec<Snowflake> = folder
            .guilds
            .iter()
            .map(|&g| Snowflake(g))
            .filter(|g| known.contains(g) && !placed.contains(g))
            .collect();
        placed.extend(&guilds);
        match folder.id {
            Some(id) if !guilds.is_empty() => entries.push(Entry::Folder {
                key: id as u64,
                name: folder.name.clone(),
                color: folder.color,
                guilds,
            }),
            _ => entries.extend(guilds.into_iter().map(Entry::Guild)),
        }
    }

    let mut out: Vec<Entry> = known.iter().filter(|g| !placed.contains(g)).map(|&g| Entry::Guild(g)).collect();
    out.extend(entries);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(id: Option<i64>, guilds: &[u64]) -> GuildFolder {
        GuildFolder { id, name: None, color: None, guilds: guilds.to_vec() }
    }

    #[test]
    fn folders_group_their_servers_and_lone_servers_stay_plain() {
        let known = [Snowflake(1), Snowflake(2), Snowflake(3)];
        let out = arrange(&[folder(Some(9), &[2, 3]), folder(None, &[1])], &known);
        assert_eq!(
            out,
            vec![
                Entry::Folder { key: 9, name: None, color: None, guilds: vec![Snowflake(2), Snowflake(3)] },
                Entry::Guild(Snowflake(1)),
            ]
        );
    }

    #[test]
    fn servers_we_left_vanish_and_new_ones_come_first() {
        let known = [Snowflake(1), Snowflake(4)];
        let out = arrange(&[folder(Some(9), &[1, 2]), folder(Some(8), &[3])], &known);
        assert_eq!(
            out,
            vec![
                Entry::Guild(Snowflake(4)),
                Entry::Folder { key: 9, name: None, color: None, guilds: vec![Snowflake(1)] },
            ]
        );
    }

    #[test]
    fn without_an_arrangement_the_account_order_is_kept() {
        let known = [Snowflake(5), Snowflake(6)];
        assert_eq!(arrange(&[], &known), vec![Entry::Guild(Snowflake(5)), Entry::Guild(Snowflake(6))]);
    }
}
