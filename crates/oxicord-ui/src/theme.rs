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
/// channeltextarea-background.
pub const BG_INPUT: u32 = 0x131416;

/// border-subtle (the app frame line) over black.
pub const BORDER: u32 = 0x1e1e20;
/// text-default.
pub const TEXT: u32 = 0xd4d5d8;
/// text-strong.
pub const TEXT_BRIGHT: u32 = 0xdcdcdf;
/// interactive-text-default, the grey of idle channel names and icons.
pub const TEXT_MUTED: u32 = 0x96979e;
/// text-muted.
pub const TEXT_FAINT: u32 = 0x81828a;

/// background-brand.
pub const ACCENT: u32 = 0x5865f2;
/// control-critical-secondary-text-default.
pub const DANGER: u32 = 0xeb5f5e;
/// background-feedback-notification.
pub const BADGE: u32 = 0xd22d39;
pub const ONLINE: u32 = 0x3d9e60;
pub const IDLE: u32 = 0xfdb833;
pub const DND: u32 = 0xda3e44;
/// text-status-offline.
pub const OFFLINE: u32 = 0x9d9ea5;

pub const RAIL_WIDTH: f32 = 72.0;
pub const SIDEBAR_WIDTH: f32 = 240.0;
pub const MEMBERS_WIDTH: f32 = 240.0;
/// radius-sm. The official scale is 4, 8, 12, 16 and 24.
pub const RADIUS: f32 = 8.0;
pub const CHANNEL_HEADER_HEIGHT: f32 = 49.0;
