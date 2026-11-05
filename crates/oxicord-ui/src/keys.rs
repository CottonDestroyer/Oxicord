//! App wide keyboard shortcuts. One table drives both the key bindings and
//! the Keybinds settings page, so the two can't disagree.

use gpui_kit::{Action, App, KeyBinding};

gpui_kit::actions!(
    oxicord,
    [
        QuickSwitch,
        PrevChannel,
        NextChannel,
        PrevUnread,
        NextUnread,
        PrevServer,
        NextServer,
        MarkChannelRead,
        MarkServerRead,
        TogglePins,
        ToggleInbox,
        ToggleMembers,
        Search,
        EmojiPicker,
        GifPicker,
        StickerPicker,
        Upload,
        NewGroupDm,
        ShowKeybinds,
    ]
);

pub struct Shortcut {
    pub group: &'static str,
    pub label: &'static str,
    /// In gpui's notation.
    pub keys: &'static str,
    binding: fn(&'static str, Option<&'static str>) -> KeyBinding,
    /// Text fields bind this key themselves, so we bind it again inside them
    /// to win. Only for keys that mean nothing while typing a message.
    over_text: bool,
}

fn to<A: Action + Default>(keys: &'static str, context: Option<&'static str>) -> KeyBinding {
    KeyBinding::new(keys, A::default(), context)
}

/// Discord's main modifier: Ctrl, or Cmd on a Mac.
macro_rules! m {
    ($rest:literal) => {
        if cfg!(target_os = "macos") { concat!("cmd-", $rest) } else { concat!("ctrl-", $rest) }
    };
}

const fn shortcut(
    group: &'static str,
    label: &'static str,
    keys: &'static str,
    binding: fn(&'static str, Option<&'static str>) -> KeyBinding,
) -> Shortcut {
    Shortcut { group, label, keys, binding, over_text: false }
}

const fn over_text(mut s: Shortcut) -> Shortcut {
    s.over_text = true;
    s
}

/// Discord's defaults for what this app can do.
pub static SHORTCUTS: &[Shortcut] = &[
    shortcut("Navigation", "Quick switcher", m!("k"), to::<QuickSwitch>),
    shortcut("Navigation", "Previous channel", "alt-up", to::<PrevChannel>),
    shortcut("Navigation", "Next channel", "alt-down", to::<NextChannel>),
    shortcut("Navigation", "Previous unread channel", "alt-shift-up", to::<PrevUnread>),
    shortcut("Navigation", "Next unread channel", "alt-shift-down", to::<NextUnread>),
    over_text(shortcut("Navigation", "Previous server", m!("alt-up"), to::<PrevServer>)),
    over_text(shortcut("Navigation", "Next server", m!("alt-down"), to::<NextServer>)),
    shortcut("Navigation", "Create a group DM", m!("shift-t"), to::<NewGroupDm>),
    shortcut("Messages", "Mark channel read", "escape", to::<MarkChannelRead>),
    shortcut("Messages", "Mark server read", "shift-escape", to::<MarkServerRead>),
    shortcut("Messages", "Emoji picker", m!("e"), to::<EmojiPicker>),
    shortcut("Messages", "GIF picker", m!("g"), to::<GifPicker>),
    shortcut("Messages", "Sticker picker", m!("s"), to::<StickerPicker>),
    shortcut("Messages", "Upload a file", m!("shift-u"), to::<Upload>),
    shortcut("Panels", "Pinned messages", m!("p"), to::<TogglePins>),
    shortcut("Panels", "Inbox", m!("i"), to::<ToggleInbox>),
    shortcut("Panels", "Member list", m!("u"), to::<ToggleMembers>),
    over_text(shortcut("Panels", "Search", m!("f"), to::<Search>)),
    shortcut("Panels", "Keyboard shortcuts", m!("/"), to::<ShowKeybinds>),
];

/// The text fields' key context, see [`Shortcut::over_text`].
const TEXT_CONTEXT: &str = "Input";

pub fn bind(cx: &mut App) {
    let mut bindings = Vec::new();
    for s in SHORTCUTS {
        bindings.push((s.binding)(s.keys, None));
        if s.over_text {
            bindings.push((s.binding)(s.keys, Some(TEXT_CONTEXT)));
        }
    }
    cx.bind_keys(bindings);
}

/// How a key combination is written on screen, one entry per key cap.
pub fn caps(keys: &str) -> Vec<String> {
    keys.split('-')
        .map(|part| match part {
            "cmd" => "Cmd".to_owned(),
            "ctrl" => "Ctrl".to_owned(),
            "alt" => "Alt".to_owned(),
            "shift" => "Shift".to_owned(),
            "escape" => "Esc".to_owned(),
            "up" => "\u{2191}".to_owned(),
            "down" => "\u{2193}".to_owned(),
            other => other.to_uppercase(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_written_as_caps() {
        let ctrl = if cfg!(target_os = "macos") { "Cmd" } else { "Ctrl" };
        assert_eq!(caps(m!("alt-up")), [ctrl, "Alt", "\u{2191}"]);
        assert_eq!(caps("shift-escape"), ["Shift", "Esc"]);
        assert_eq!(caps(m!("/")), [ctrl, "/"]);
    }

    #[test]
    fn every_key_parses() {
        for s in SHORTCUTS {
            assert!(gpui_kit::Keystroke::parse(s.keys).is_ok(), "{} does not parse", s.keys);
        }
    }

    #[test]
    fn no_key_is_bound_twice() {
        let mut seen = std::collections::HashSet::new();
        for s in SHORTCUTS {
            assert!(seen.insert(s.keys), "{} is bound twice", s.keys);
        }
    }
}
