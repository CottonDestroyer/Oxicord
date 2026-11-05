use gpui_kit::{
    AnyElement, App, ClickEvent, FontWeight, InteractiveElement as _, IntoElement, Entity,
    ParentElement as _, StatefulInteractiveElement as _, Styled as _, Window, div, px, rgb, rgba,
};
use oxicord_model::Status;

use crate::{
    icons::icon,
    media::{Media, avatar},
    member_list::presence_dot,
    theme,
};

/// Everything the card shows, gathered by the caller so this file doesn't
/// need to know about the store.
pub struct Card {
    pub name: String,
    pub username: String,
    pub pronouns: String,
    pub avatar_url: String,
    pub status: Status,
    /// Empty until the profile arrives, or when the person wrote none.
    pub bio: String,
    pub created: String,
    /// Role name and color, highest role first. Empty outside servers.
    pub roles: Vec<(String, Option<u32>)>,
    /// Hide the message button on our own card.
    pub is_me: bool,
}

pub type Handler = dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static;

/// A dimmed overlay with the card centered on it. Clicking outside closes it.
pub fn overlay(
    card: Card,
    media: &Entity<Media>,
    on_message: Box<Handler>,
    on_close: Box<Handler>,
    cx: &mut App,
) -> AnyElement {
    modal(body(card, media, on_message, cx), on_close)
}

/// Dims everything behind `content` and closes when the dimmed part is
/// clicked. `content` should call `.occlude()` so clicks on it stay on it.
pub fn modal(content: AnyElement, on_close: Box<Handler>) -> AnyElement {
    div()
        .id("profile-overlay")
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .id("profile-scrim")
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .bg(rgba(0x0000_0099))
                .on_click(on_close),
        )
        .child(content)
        .into_any_element()
}

fn body(card: Card, media: &Entity<Media>, on_message: Box<Handler>, cx: &mut App) -> AnyElement {
    let status = match card.status {
        Status::Online => "Online",
        Status::Idle => "Idle",
        Status::Dnd => "Do not disturb",
        Status::Offline => "Offline",
    };

    let mut sections = div().flex().flex_col().gap_3().p_3().rounded(px(8.)).bg(rgb(theme::BG_PANEL));
    if !card.bio.is_empty() {
        sections = sections.child(section("About me", div().child(card.bio)));
    }
    if !card.roles.is_empty() {
        let mut chips = div().flex().flex_wrap().gap_1();
        for (name, color) in card.roles {
            chips = chips.child(
                div()
                    .h(px(22.))
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_1()
                    .rounded(px(4.))
                    .bg(rgb(theme::BG_ACTIVE))
                    .text_size(px(12.))
                    .child(div().size(px(10.)).rounded_full().bg(rgb(color.unwrap_or(theme::OFFLINE))))
                    .child(name),
            );
        }
        sections = sections.child(section("Roles", chips));
    }
    sections = sections.child(section("Joined Discord", div().child(card.created)));

    div()
        .id("profile-card")
        .occlude()
        .relative()
        .w(px(340.))
        .rounded(px(10.))
        .overflow_hidden()
        .border_1()
        .border_color(rgb(theme::BORDER))
        .bg(rgb(theme::BG_SIDEBAR))
        .child(div().h(px(72.)).w_full().bg(rgb(theme::ACCENT)))
        .child(
            div()
                .px_4()
                .pb_4()
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    div()
                        .flex()
                        .items_end()
                        .justify_between()
                        .child(
                            div()
                                .relative()
                                .mt(px(-40.))
                                .rounded_full()
                                .border_4()
                                .border_color(rgb(theme::BG_SIDEBAR))
                                .bg(rgb(theme::BG_SIDEBAR))
                                .child(avatar(media, &card.avatar_url, 80., &card.name, cx))
                                .child(presence_dot(card.status)),
                        )
                        .children((!card.is_me).then(|| {
                            div()
                                .id("profile-message")
                                .h(px(32.))
                                .px_3()
                                .mt_2()
                                .flex()
                                .items_center()
                                .gap_2()
                                .rounded(px(6.))
                                .cursor_pointer()
                                .bg(rgb(theme::ACCENT))
                                .text_color(rgb(theme::TEXT_BRIGHT))
                                .hover(|s| s.opacity(0.85))
                                .on_click(on_message)
                                .child(icon("message-circle", 16., theme::TEXT_BRIGHT))
                                .child("Message")
                        })),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .text_size(px(20.))
                                .font_weight(FontWeight::BOLD)
                                .text_color(rgb(theme::TEXT_BRIGHT))
                                .child(card.name),
                        )
                        .child(
                            div()
                                .text_size(px(13.))
                                .text_color(rgb(theme::TEXT_MUTED))
                                .child(if card.pronouns.is_empty() {
                                    card.username
                                } else {
                                    format!("{} \u{b7} {}", card.username, card.pronouns)
                                }),
                        )
                        .child(
                            div()
                                .mt_1()
                                .text_size(px(12.))
                                .text_color(rgb(theme::TEXT_FAINT))
                                .child(status),
                        ),
                )
                .child(sections),
        )
        .into_any_element()
}

fn section(title: &'static str, content: impl IntoElement) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .text_size(px(11.))
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(theme::TEXT_MUTED))
                .child(title.to_uppercase()),
        )
        .child(div().text_size(px(14.)).text_color(rgb(theme::TEXT)).child(content))
}
