//! The strip across the top of the window that stands in for the system title
//! bar: a drag area with the name of what is open, and the caption buttons.
//!
//! On Windows the areas are only marked, the system does the rest: dragging,
//! double click to maximize, Snap and the snap layouts on the maximize button.

use gpui_kit::{
    AnyElement, App, ClickEvent, InteractiveElement as _, IntoElement, ParentElement as _, SharedString,
    StatefulInteractiveElement as _, Styled as _, Window, WindowControlArea, div, prelude::FluentBuilder as _, px,
    rgb,
};

use crate::{icons::icon, theme};

pub const HEIGHT: f32 = 32.;
const BUTTON_WIDTH: f32 = 46.;
/// The red Windows itself uses for a hovered and a pressed close button.
const CLOSE_HOVER: u32 = 0xc42b1c;
const CLOSE_ACTIVE: u32 = 0x8b0a14;
/// Room for the traffic lights, which macOS keeps drawing itself.
const MAC_INSET: f32 = 72.;

/// The two glyphs at the right end: Inbox and Help, 24px boxes with 18px
/// glyphs, 12px apart and 12px in from the edge.
const TRAILING_WIDTH: f32 = 24. + 12. + 24. + 12.;

/// Lays over the top of the window, so put it last among the root's children
/// and leave [`HEIGHT`] free above the content. `lead` is the 16px or 18px
/// picture before the name. The Inbox and Help glyphs show once there is a
/// handler for Inbox, so not on the sign in screen.
pub fn title_bar(
    title: SharedString,
    lead: Option<AnyElement>,
    on_inbox: Option<Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>>,
    window: &Window,
) -> impl IntoElement {
    let mac = cfg!(target_os = "macos");
    // The name sits on the middle of the window, so the side without the
    // glyphs is padded by what the other side takes.
    let side = if on_inbox.is_some() { TRAILING_WIDTH } else { 0. };
    let balance = if mac { MAC_INSET } else { side + 3. * BUTTON_WIDTH };
    let maximize = if window.is_maximized() { "window-restore" } else { "window-maximize" };

    div()
        .id("title-bar")
        .absolute()
        .top_0()
        .left_0()
        .right_0()
        .h(px(HEIGHT))
        .flex()
        .items_center()
        .bg(rgb(theme::BG_RAIL))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .h_full()
                .pl(px(balance))
                .flex()
                .items_center()
                .justify_center()
                .window_control_area(WindowControlArea::Drag)
                .child(
                    div()
                        .min_w_0()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .children(lead)
                        .child(
                            div()
                                .truncate()
                                .text_size(px(14.))
                                .line_height(px(18.))
                                .font_weight(gpui_kit::FontWeight::MEDIUM)
                                .text_color(rgb(theme::TEXT_BRIGHT))
                                .child(title),
                        ),
                ),
        )
        .children(on_inbox.map(|on_inbox| {
            div()
                .w(px(TRAILING_WIDTH))
                .h_full()
                .flex_none()
                .flex()
                .items_center()
                .gap(px(12.))
                .pr(px(12.))
                .child(trailing("title-inbox", "inbox").on_click(on_inbox))
                .child(trailing("title-help", "circle-question-mark").on_click(|_, _, cx| {
                    cx.open_url("https://support.discord.com")
                }))
        }))
        .when(!mac, |bar| {
            bar.child(caption("minimize", "window-minimize", WindowControlArea::Min))
                .child(caption("maximize", maximize, WindowControlArea::Max))
                .child(caption("close", "window-close", WindowControlArea::Close))
        })
}

/// An 18px glyph in a 24px box, muted until the pointer is over it.
fn trailing(id: &'static str, glyph: &'static str) -> gpui_kit::Stateful<gpui_kit::Div> {
    div()
        .id(id)
        .group(id)
        .size(px(24.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .child(icon(glyph, 18., theme::TEXT_MUTED).group_hover(id, |s| s.text_color(rgb(theme::TEXT_BRIGHT))))
}

fn caption(id: &'static str, glyph: &'static str, area: WindowControlArea) -> impl IntoElement {
    let close = area == WindowControlArea::Close;
    div()
        .id(id)
        .w(px(BUTTON_WIDTH))
        .h_full()
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .window_control_area(area)
        .hover(|s| s.bg(rgb(if close { CLOSE_HOVER } else { theme::BG_HOVER })))
        .active(|s| s.bg(rgb(if close { CLOSE_ACTIVE } else { theme::BG_ACTIVE })))
        // Windows answers clicks on these areas itself. Elsewhere nothing
        // does, so the buttons act on their own.
        .when(!cfg!(target_os = "windows"), |button| {
            button.on_click(move |_, window, _| match area {
                WindowControlArea::Min => window.minimize_window(),
                WindowControlArea::Max => window.zoom_window(),
                _ => window.remove_window(),
            })
        })
        .child(icon(glyph, 16., theme::TEXT))
}
