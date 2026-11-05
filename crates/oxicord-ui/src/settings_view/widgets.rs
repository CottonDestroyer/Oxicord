//! Small building blocks shared by the settings pages.

use gpui_kit::{
    AnyElement, App, ClickEvent, FontWeight, InteractiveElement as _, IntoElement,
    ParentElement as _, StatefulInteractiveElement as _, Styled as _, Window, div, px, rgb,
};

use crate::{icons::icon, theme};

pub fn title(text: &'static str) -> impl IntoElement {
    div()
        .mb_2()
        .text_size(px(20.))
        .font_weight(FontWeight::BOLD)
        .text_color(rgb(theme::TEXT_BRIGHT))
        .child(text)
}

pub fn blurb(text: &'static str) -> impl IntoElement {
    div()
        .mb_2()
        .text_size(px(13.))
        .text_color(rgb(theme::TEXT_MUTED))
        .child(text)
}

/// A labelled block of related controls on a darker card.
pub fn group(heading: &'static str, rows: Vec<AnyElement>) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .mt_3()
                .text_size(px(11.))
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(theme::TEXT_MUTED))
                .child(heading.to_uppercase()),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_3()
                .p_3()
                .rounded(px(8.))
                .bg(rgb(theme::BG_PANEL))
                .children(rows),
        )
}

pub fn row_text(label: &str, hint: Option<&str>) -> impl IntoElement {
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .child(div().text_color(rgb(theme::TEXT_BRIGHT)).child(label.to_owned()))
        .children(hint.map(|hint| {
            div()
                .text_size(px(12.))
                .text_color(rgb(theme::TEXT_MUTED))
                .child(hint.to_owned())
        }))
}

pub fn switch_row(
    id: &'static str,
    label: &'static str,
    hint: Option<&'static str>,
    on: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> AnyElement {
    let track = div()
        .flex_none()
        .w(px(40.))
        .h(px(22.))
        .p(px(3.))
        .flex()
        .rounded_full()
        .bg(rgb(if on { theme::ONLINE } else { theme::BG_ACTIVE }))
        .child(div().size(px(16.)).rounded_full().bg(rgb(theme::TEXT_BRIGHT)));
    // The knob sits at the end when the switch is on.
    let track = if on { track.justify_end() } else { track };

    div()
        .id(id)
        .flex()
        .items_center()
        .gap_3()
        .cursor_pointer()
        .on_click(on_click)
        .child(row_text(label, hint))
        .child(track)
        .into_any_element()
}

/// One choice of a set where exactly one applies.
pub fn radio_row(
    id: (&'static str, u32),
    label: &'static str,
    hint: Option<&'static str>,
    selected: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> AnyElement {
    let dot = div()
        .flex_none()
        .size(px(18.))
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .border_2()
        .border_color(rgb(if selected { theme::ACCENT } else { theme::TEXT_FAINT }))
        .children(selected.then(|| div().size(px(8.)).rounded_full().bg(rgb(theme::ACCENT))));

    div()
        .id(id)
        .flex()
        .items_center()
        .gap_3()
        .cursor_pointer()
        .on_click(on_click)
        .child(dot)
        .child(row_text(label, hint))
        .into_any_element()
}

pub fn info_row(label: &'static str, value: String) -> AnyElement {
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .child(
            div()
                .text_size(px(12.))
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(theme::TEXT_MUTED))
                .child(label.to_uppercase()),
        )
        .child(div().truncate().text_color(rgb(theme::TEXT)).child(value))
        .into_any_element()
}

pub enum Tone {
    Primary,
    Plain,
    Danger,
}

pub fn button(
    id: impl Into<gpui_kit::ElementId>,
    label: &'static str,
    tone: Tone,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> AnyElement {
    let (fill, border, text) = match tone {
        Tone::Primary => (theme::ACCENT, theme::ACCENT, theme::TEXT_BRIGHT),
        Tone::Plain => (theme::BG_ACTIVE, theme::BG_ACTIVE, theme::TEXT_BRIGHT),
        Tone::Danger => (theme::BG_PANEL, theme::DANGER, theme::DANGER),
    };
    // The wrapper keeps the button its natural width inside a column.
    div()
        .flex()
        .flex_none()
        .child(
            div()
                .id(id)
                .h(px(32.))
                .px_3()
                .flex()
                .items_center()
                .rounded(px(6.))
                .border_1()
                .border_color(rgb(border))
                .bg(rgb(fill))
                .text_color(rgb(text))
                .cursor_pointer()
                .hover(|s| s.opacity(0.85))
                .on_click(on_click)
                .child(label),
        )
        .into_any_element()
}

/// A line of muted text for empty, loading and failed lists.
pub fn status_line(glyph: &str, text: String, color: u32) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap_2()
        .text_color(rgb(color))
        .child(icon(glyph, 16., color))
        .child(text)
        .into_any_element()
}
