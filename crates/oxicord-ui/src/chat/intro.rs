//! The block at the very top of a conversation, shown once the whole history
//! is loaded: who the chat is with, or which channel this is.

use gpui_kit::{
    AnyElement, App, Entity, FontWeight, IntoElement, ParentElement as _, Styled as _, div, px, rgb,
};

use crate::{icons::icon, media::{Media, avatar}, theme};

/// The start of a direct message.
pub fn direct(media: &Entity<Media>, avatar_url: &str, name: &str, username: &str, cx: &mut App) -> AnyElement {
    let body = div()
        .flex()
        .flex_wrap()
        .text_size(px(16.))
        .line_height(px(20.))
        .text_color(rgb(theme::TEXT))
        .child("This is the beginning of your direct message history with ")
        .child(div().font_weight(FontWeight::SEMIBOLD).child(name.to_owned()))
        .child(".");

    frame()
        .child(avatar(media, avatar_url, 80., name, cx))
        .child(heading(name))
        .child(
            div()
                .mb(px(20.))
                .text_size(px(24.))
                .line_height(px(32.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(rgb(theme::TEXT_BRIGHT))
                .child(username.to_owned()),
        )
        .child(body)
        .into_any_element()
}

/// The start of a group conversation.
pub fn group(name: &str) -> AnyElement {
    frame()
        .child(heading(name))
        .child(
            div()
                .text_size(px(16.))
                .line_height(px(20.))
                .text_color(rgb(theme::TEXT))
                .child("Welcome to the beginning of the group."),
        )
        .into_any_element()
}

/// The start of a server channel.
pub fn channel(name: &str, topic: Option<&str>) -> AnyElement {
    let mut block = frame()
        .child(
            div()
                .size(px(68.))
                .flex()
                .items_center()
                .justify_center()
                .rounded_full()
                .bg(rgb(theme::BG_ACTIVE))
                .child(icon("hash", 42., theme::TEXT_BRIGHT)),
        )
        .child(heading(&format!("Welcome to #{name}!")))
        .child(
            div()
                .text_size(px(16.))
                .line_height(px(20.))
                .text_color(rgb(theme::TEXT))
                .child(format!("This is the start of the #{name} channel.")),
        );
    if let Some(topic) = topic.filter(|t| !t.is_empty()) {
        block = block.child(
            div().mt(px(4.)).text_size(px(14.)).text_color(rgb(theme::TEXT_MUTED)).child(topic.to_owned()),
        );
    }
    block.into_any_element()
}

fn frame() -> gpui_kit::Div {
    div().flex().flex_col().items_start().mx(px(16.)).mt(px(16.)).mb(px(8.))
}

fn heading(text: &str) -> impl IntoElement {
    div()
        .my(px(8.))
        .text_size(px(32.))
        .line_height(px(40.))
        .font_weight(FontWeight::BOLD)
        .text_color(rgb(theme::TEXT_BRIGHT))
        .child(text.to_owned())
}
