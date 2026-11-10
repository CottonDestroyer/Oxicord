//! Color tokens for Discord's Midnight theme. Every surface is pure black and
//! panes are told apart by thin borders, so the translucent overlays the
//! official client draws (hover, selected, border) are flattened here onto
//! black. Each value names the token it was read from.

/// background-base-lowest, -lower and -low are all the same black.
pub const BG_RAIL: u32 = 0x000000;
pub const BG_SIDEBAR: u32 = 0x000000;
pub const BG_CHAT: u32 = 0x000000;
pub const BG_MEMBERS: u32 = 0x000000;
/// background-surface-high: dialogs, menus and cards.
pub const BG_PANEL: u32 = 0x0a0a0c;
/// interactive-background-hover over black.
pub const BG_HOVER: u32 = 0x121213;
/// interactive-background-selected over black.
pub const BG_ACTIVE: u32 = 0x242426;
/// The composer box, read from the live client.
pub const BG_INPUT: u32 = 0x101013;

/// border-subtle (the app frame line) over black.
pub const BORDER: u32 = 0x1e1e20;
/// border-muted over black, the outline of floating cards.
pub const BORDER_MUTED: u32 = 0x18181a;
/// text-default.
pub const TEXT: u32 = 0xd4d5d8;
/// text-strong.
pub const TEXT_BRIGHT: u32 = 0xdcdcdf;
/// interactive-text-default, the grey of idle channel names and icons.
pub const TEXT_MUTED: u32 = 0x96979e;
/// text-muted.
pub const TEXT_FAINT: u32 = 0x81828a;
/// background-surface-higher: menus and popouts.
pub const BG_MENU: u32 = 0x121214;
/// The guild card in the screening dialog, one step above the dialog.
pub const BG_CARD: u32 = 0x17181b;
/// background-feedback-positive, the screening dialog's submit button.
pub const SUCCESS: u32 = 0x008545;
/// The same button under the pointer.
pub const SUCCESS_HOVER: u32 = 0x006c37;
/// control-checked-background-default, the screening dialog's checkbox.
pub const CHECKBOX: u32 = 0x4452bb;
/// background-feedback-mod-critical over a menu, behind a focused danger item.
pub const DANGER_HOVER: u32 = 0x231819;
/// status-positive-background, the add server button on hover.
pub const ADD_SERVER_HOVER: u32 = 0x008043;
/// chat-text-muted, message timestamps.
pub const CHAT_TEXT_MUTED: u32 = 0x73747d;
/// message-highlight-background-default over black.
pub const MESSAGE_HIGHLIGHT: u32 = 0x0e1027;
/// channels-default, the idle color of channel and DM names.
pub const CHANNELS_DEFAULT: u32 = 0x7a7b83;
/// interactive-muted, how muted channels are drawn.
pub const CHANNEL_MUTED: u32 = 0x4f505a;

/// background-brand.
pub const ACCENT: u32 = 0x5865f2;
/// control-primary-background-hover, the brand button under the pointer.
pub const ACCENT_HOVER: u32 = 0x4452bb;
/// control-secondary-background-default over the dialog background.
pub const SECONDARY: u32 = 0x1b1b1e;
/// control-secondary-background-hover over the dialog background.
pub const SECONDARY_HOVER: u32 = 0x262629;
/// border-subtle over the dialog background, rgba(150,151,159,0.24).
pub const BORDER_OVERLAY: u32 = 0x2c2c2e;
/// The modal frame: border-subtle at 0.2 over `BG_PANEL`.
pub const MODAL_BORDER: u32 = 0x25252a;
/// control-critical-secondary-text-default.
pub const DANGER: u32 = 0xeb5f5e;
/// background-feedback-mod-critical (rgba(242,63,67,0.1)) over `BG_PANEL`.
pub const DANGER_BANNER: u32 = 0x210f10;
/// The matching critical border, rgba(242,63,67,0.4) over `BG_PANEL`.
pub const DANGER_BORDER: u32 = 0x671f22;
/// background-feedback-notification.
pub const BADGE: u32 = 0xd22d39;
/// status-positive: the connected label, the voice channel you are in and
/// the ring around whoever is talking.
pub const STATUS_POSITIVE: u32 = 0x41a063;
/// A call tile before its owner's color is known.
pub const CALL_TILE: u32 = 0x1e1f22;
pub const ONLINE: u32 = 0x3d9e60;
pub const IDLE: u32 = 0xfdb833;
pub const DND: u32 = 0xda3e44;
/// text-status-offline.
pub const OFFLINE: u32 = 0x9d9ea5;

static GG_SANS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Called once the gg sans files are registered with the text system.
pub fn gg_sans_loaded() {
    GG_SANS.store(true, std::sync::atomic::Ordering::Relaxed);
}

/// The family every screen draws in: Discord's own typeface when its files
/// were found, the system font otherwise.
pub fn font_family() -> &'static str {
    if GG_SANS.load(std::sync::atomic::Ordering::Relaxed) {
        "gg sans"
    } else {
        ".SystemUIFont"
    }
}

pub const RAIL_WIDTH: f32 = 72.0;
pub const SIDEBAR_WIDTH: f32 = 293.0;
pub const MEMBERS_WIDTH: f32 = 264.0;
/// radius-sm. The official scale is 4, 8, 12, 16 and 24.
pub const RADIUS: f32 = 8.0;
/// radius-md, the corner of the main frame.
pub const RADIUS_MD: f32 = 12.0;
pub const CHANNEL_HEADER_HEIGHT: f32 = 49.0;
/// header-primary, the colour of channel titles in the header bar.
pub const HEADER_PRIMARY: u32 = 0xfbfbfb;
