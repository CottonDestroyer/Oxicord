//! App wide keyboard shortcuts.

use gpui_kit::{App, KeyBinding};

gpui_kit::actions!(oxicord, [QuickSwitch]);

pub fn bind(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("ctrl-k", QuickSwitch, None), KeyBinding::new("cmd-k", QuickSwitch, None)]);
}
