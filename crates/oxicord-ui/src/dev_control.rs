//! Debug builds only. Lets a script steer the running app by dropping lines
//! into a text file, so checking a screen does not mean restarting the app
//! and hunting for it with the mouse.
//!
//! One command per line:
//!
//! ```text
//! goto <channel id>   open any channel we know, its server included
//! join <channel id>   open it and join it as a voice channel
//! leave               leave the call
//! mute                toggle mute
//! deafen              toggle deafen
//! profile <user|me> [server id]   the small card, as if clicked in the member list
//! full <user> [server id]      the full profile
//! tab <board|activity|friends|servers>   switch the full profile's tab
//! close               close the card
//! ```
//!
//! The file is `OXICORD_DEV_CMD`, or `oxicord-dev-cmd.txt` in the temp folder.
//! It is read and deleted every half second.

use std::path::PathBuf;

use oxicord_model::Snowflake;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DevCommand {
    Goto(Snowflake),
    Join(Snowflake),
    Leave,
    Mute,
    Deafen,
    Profile(Snowflake, Option<Snowflake>),
    Full(Snowflake, Option<Snowflake>),
    /// The membership screening dialog for a server.
    Rules(Snowflake),
    /// 0 board, 1 activity, 2 friends, 3 servers.
    Tab(u8),
    /// A synthetic message request channel, so its row and page can be
    /// checked without a real stranger writing first.
    FakeRequest(Snowflake, Snowflake),
    FakeSpam(Snowflake, Snowflake),
    /// The bot modal fixture, for checking its layout without a live click.
    Modal,
    Close,
    Resize(u32, u32),
}

pub fn command_path() -> PathBuf {
    std::env::var_os("OXICORD_DEV_CMD")
        .map_or_else(|| std::env::temp_dir().join("oxicord-dev-cmd.txt"), PathBuf::from)
}

pub fn parse(line: &str) -> Option<DevCommand> {
    let mut words = line.split_whitespace();
    let command = words.next()?;
    // `me` is id 0, which the app swaps for our own account.
    let id = || {
        let word = words.clone().next()?;
        if word == "me" { Some(Snowflake(0)) } else { word.parse().ok().map(Snowflake) }
    };
    let second = || words.clone().nth(1)?.parse().ok().map(Snowflake);
    match command {
        "profile" => id().map(|user| DevCommand::Profile(user, second())),
        "full" => id().map(|user| DevCommand::Full(user, second())),
        "rules" => id().map(DevCommand::Rules),
        "tab" => match words.clone().next()? {
            "board" => Some(DevCommand::Tab(0)),
            "activity" => Some(DevCommand::Tab(1)),
            "friends" => Some(DevCommand::Tab(2)),
            "servers" => Some(DevCommand::Tab(3)),
            _ => None,
        },
        "close" => Some(DevCommand::Close),
        "resize" => {
            let w: u32 = words.clone().next()?.parse().ok()?;
            let h: u32 = words.clone().nth(1)?.parse().ok()?;
            Some(DevCommand::Resize(w, h))
        }
        "goto" => id().map(DevCommand::Goto),
        "join" => id().map(DevCommand::Join),
        "fake-request" => {
            let channel = words.clone().next()?.parse().ok().map(Snowflake)?;
            let user = words.clone().nth(1)?.parse().ok().map(Snowflake)?;
            Some(DevCommand::FakeRequest(channel, user))
        }
        "fake-spam" => {
            let channel = words.clone().next()?.parse().ok().map(Snowflake)?;
            let user = words.clone().nth(1)?.parse().ok().map(Snowflake)?;
            Some(DevCommand::FakeSpam(channel, user))
        }
        "modal" => Some(DevCommand::Modal),
        "leave" => Some(DevCommand::Leave),
        "mute" => Some(DevCommand::Mute),
        "deafen" => Some(DevCommand::Deafen),
        _ => None,
    }
}

/// Reads and clears the command file.
pub fn take() -> Vec<DevCommand> {
    let path = command_path();
    let Ok(text) = std::fs::read_to_string(&path) else { return Vec::new() };
    let _ = std::fs::remove_file(&path);
    text.lines().filter_map(parse).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_are_read_one_per_line() {
        assert_eq!(parse("goto 123"), Some(DevCommand::Goto(Snowflake(123))));
        assert_eq!(parse("  join   9 "), Some(DevCommand::Join(Snowflake(9))));
        assert_eq!(parse("leave"), Some(DevCommand::Leave));
        assert_eq!(parse("mute"), Some(DevCommand::Mute));
        assert_eq!(parse("deafen"), Some(DevCommand::Deafen));
    }

    #[test]
    fn profile_commands_take_a_person_and_an_optional_server() {
        assert_eq!(parse("profile 5"), Some(DevCommand::Profile(Snowflake(5), None)));
        assert_eq!(parse("full 5 9"), Some(DevCommand::Full(Snowflake(5), Some(Snowflake(9)))));
        assert_eq!(parse("profile me"), Some(DevCommand::Profile(Snowflake(0), None)));
        assert_eq!(parse("rules 9"), Some(DevCommand::Rules(Snowflake(9))));
        assert_eq!(parse("tab servers"), Some(DevCommand::Tab(3)));
        assert_eq!(parse("tab nowhere"), None);
        assert_eq!(parse("close"), Some(DevCommand::Close));
    }

    #[test]
    fn nonsense_is_ignored() {
        assert_eq!(parse(""), None);
        assert_eq!(parse("goto"), None);
        assert_eq!(parse("goto abc"), None);
        assert_eq!(parse("fly 3"), None);
    }
}
