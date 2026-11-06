//! The strip across the top of the window that stands in for the system title
//! bar: a drag area with the name of what is open, and the caption buttons.
//!
//! On Windows the areas are only marked, the system does the rest: dragging,
//! double click to maximize, Snap and the snap layouts on the maximize button.

use gpui_kit::{
    InteractiveElement as _, IntoElement, ParentElement as _, SharedString, StatefulInteractiveElement as _,
    Styled as _, Window, WindowControlArea, div, prelude::FluentBuilder as _, px, rgb,
};

use crate::{icons::icon, theme};

pub const HEIGHT: f32 = 32.;
const BUTTON_WIDTH: f32 = 46.;
/// The red Windows itself uses for a hovered and a pressed close button.
const CLOSE_HOVER: u32 = 0xc42b1c;
const CLOSE_ACTIVE: u32 = 0x8b0a14;
/// Room for the traffic lights, which macOS keeps drawing itself.
const MAC_INSET: f32 = 72.;

/// Lays over the top of the window, so put it last among the root's children
/// and leave [`HEIGHT`] free above the content.
pub fn title_bar(title: SharedString, window: &Window) -> impl IntoElement {
    let mac = cfg!(target_os = "macos");
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
                .when(mac, |s| s.pl(px(MAC_INSET)))
                .flex()
                .items_center()
                .justify_center()
                .window_control_area(WindowControlArea::Drag)
                .child(
                    div()
                        .truncate()
                        .text_size(px(13.))
                        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                        .text_color(rgb(theme::TEXT))
                        .child(title),
                ),
        )
        .when(!mac, |bar| {
            bar.child(caption("minimize", "window-minimize", WindowControlArea::Min))
                .child(caption("maximize", maximize, WindowControlArea::Max))
                .child(caption("close", "window-close", WindowControlArea::Close))
        })
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
