use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, ClickEvent, FontWeight, InteractiveElement as _, IntoElement, Entity, StyledImage as _, img,
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
    /// A banner picture, or `None` to fill with the accent color.
    pub banner_url: Option<String>,
    pub accent: u32,
    /// Badge description and icon address.
    pub badges: Vec<(String, String)>,
    /// When they joined the server the card was opened from.
    pub joined_server: String,
    /// Servers in common: name and icon address.
    pub mutual_servers: Vec<(String, Option<String>)>,
    pub mutual_friends: u32,
    /// Linked accounts as `kind: name`.
    pub accounts: Vec<String>,
    /// Hide the message button on our own card.
    pub is_me: bool,
    /// Buttons for the person's relationship to us: add, remove, block.
    pub actions: Vec<CardAction>,
}

/// A small button under the person's name.
pub struct CardAction {
    pub label: &'static str,
    pub danger: bool,
    pub handler: Box<Handler>,
}

pub type Handler = dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static;

/// A dimmed overlay with the card centered on it. Clicking outside closes it.
pub fn overlay(
    card: Card,
    media: &Entity<Media>,
    on_message: Box<Handler>,
    on_close: Box<Handler>,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    modal(body(card, media, on_message, false, window, cx), on_close)
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

fn body(
    card: Card,
    media: &Entity<Media>,
    on_message: Box<Handler>,
    inline: bool,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let status = match card.status {
        Status::Online => "Online",
        Status::Idle => "Idle",
        Status::Dnd => "Do not disturb",
        Status::Offline => "Offline",
    };

    let mut sections = div().flex().flex_col().gap_3().p_3().rounded(px(8.)).bg(rgb(theme::BG_MENU));
    if !card.bio.is_empty() {
        sections = sections.child(section(
            "About me",
            crate::chat::markdown::rich(&card.bio, 0xB10, media, &Default::default(), window, cx),
        ));
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
    let mut since = div().flex().flex_col().child(div().child(format!("Discord: {}", card.created)));
    if !card.joined_server.is_empty() {
        since = since.child(div().child(format!("This server: {}", card.joined_server)));
    }
    sections = sections.child(section("Member since", since));
    if !card.mutual_servers.is_empty() {
        let mut list = div().flex().flex_col().gap_1();
        for (name, icon_url) in card.mutual_servers.iter().take(6) {
            let mut row = div().flex().items_center().gap_2();
            if let Some(url) = icon_url {
                row = row.child(avatar(media, url, 20., name, cx));
            }
            list = list.child(row.child(div().truncate().child(name.clone())));
        }
        if card.mutual_servers.len() > 6 {
            list = list.child(div().text_color(rgb(theme::TEXT_MUTED)).child(format!("and {} more", card.mutual_servers.len() - 6)));
        }
        sections = sections.child(section("Mutual servers", list));
    }
    if card.mutual_friends > 0 {
        sections = sections.child(section("Mutual friends", div().child(card.mutual_friends.to_string())));
    }
    if !card.accounts.is_empty() {
        let mut list = div().flex().flex_col();
        for line in card.accounts {
            list = list.child(div().child(line));
        }
        sections = sections.child(section("Connections", list));
    }

    div()
        .id("profile-card")
        .occlude()
        .relative()
        .w(px(300.))
        .border_color(rgb(theme::BORDER))
        .when(inline, |card| card.h_full().flex_none().overflow_y_scroll().border_l_1())
        .when(!inline, |card| card.rounded(px(theme::RADIUS)).overflow_hidden().border_1())
        .bg(rgb(theme::BG_PANEL))
        .child({
            let banner = div().h(px(105.)).w_full().bg(rgb(card.accent)).overflow_hidden();
            match card.banner_url.as_deref().and_then(|url| media.update(cx, |m, _| m.get(url))) {
                Some(image) => banner.child(
                    img(gpui_kit::ImageSource::Image(image)).size_full().object_fit(gpui_kit::ObjectFit::Cover),
                ),
                None => banner,
            }
        })
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
                                .border_color(rgb(theme::BG_PANEL))
                                .bg(rgb(theme::BG_PANEL))
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
                                .rounded(px(theme::RADIUS))
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
                .children((!card.badges.is_empty()).then(|| {
                    let mut row = div().flex().flex_wrap().gap_1();
                    for (description, url) in &card.badges {
                        let _ = description;
                        if let Some(image) = media.update(cx, |m, _| m.get(url)) {
                            row = row.child(img(gpui_kit::ImageSource::Image(image)).size(px(20.)));
                        }
                    }
                    row
                }))
                .children((!card.actions.is_empty()).then(|| {
                    let mut row = div().flex().flex_wrap().gap_2();
                    for (i, action) in card.actions.into_iter().enumerate() {
                        row = row.child(
                            div()
                                .id(("card-action", i as u64))
                                .h(px(28.))
                                .px_3()
                                .flex()
                                .items_center()
                                .rounded(px(theme::RADIUS))
                                .cursor_pointer()
                                .text_size(px(13.))
                                .border_1()
                                .border_color(rgb(if action.danger { theme::DANGER } else { theme::BORDER }))
                                .text_color(rgb(if action.danger { theme::DANGER } else { theme::TEXT }))
                                .hover(|s| s.bg(rgb(theme::BG_HOVER)))
                                .on_click(action.handler)
                                .child(action.label),
                        );
                    }
                    row
                }))
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

/// The same card docked beside a conversation instead of floating over it.
pub fn column(card: Card, media: &Entity<Media>, on_message: Box<Handler>, window: &mut Window, cx: &mut App) -> AnyElement {
    body(card, media, on_message, true, window, cx)
}
