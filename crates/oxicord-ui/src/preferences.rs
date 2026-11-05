use gpui_kit::{
    AnyElement, App, Entity, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, div, px, rgb,
};

use crate::{
    media::{Media, avatar},
    profile::{Handler, modal},
    settings::Settings,
    theme,
};

/// Who is signed in, for the account block.
pub struct Account {
    pub name: String,
    pub username: String,
    pub avatar_url: String,
}

pub struct Callbacks {
    pub close: Box<Handler>,
    pub log_out: Box<Handler>,
    pub toggle_notifications: Box<Handler>,
    pub toggle_members: Box<Handler>,
}

pub fn panel(
    account: Account,
    settings: &Settings,
    media: &Entity<Media>,
    callbacks: Callbacks,
    cx: &mut App,
) -> AnyElement {
    let Callbacks {
        close,
        log_out,
        toggle_notifications,
        toggle_members,
    } = callbacks;

    let content = div()
        .id("settings-panel")
        .occlude()
        .w(px(460.))
        .p_5()
        .flex()
        .flex_col()
        .gap_4()
        .rounded(px(10.))
        .border_1()
        .border_color(rgb(theme::BORDER))
        .bg(rgb(theme::BG_SIDEBAR))
        .child(
            div()
                .text_size(px(20.))
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(theme::TEXT_BRIGHT))
                .child("Settings"),
        )
        .child(
            group(
                "My account",
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(avatar(media, &account.avatar_url, 48., &account.name, cx))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .truncate()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(theme::TEXT_BRIGHT))
                                    .child(account.name),
                            )
                            .child(
                                div()
                                    .truncate()
                                    .text_size(px(13.))
                                    .text_color(rgb(theme::TEXT_MUTED))
                                    .child(account.username),
                            ),
                    )
                    .child(
                        div()
                            .id("settings-log-out")
                            .h(px(32.))
                            .px_3()
                            .flex()
                            .items_center()
                            .rounded(px(6.))
                            .cursor_pointer()
                            .border_1()
                            .border_color(rgb(theme::DANGER))
                            .text_color(rgb(theme::DANGER))
                            .hover(|s| s.bg(rgb(theme::BG_HOVER)))
                            .on_click(log_out)
                            .child("Log out"),
                    ),
            ),
        )
        .child(group(
            "Notifications",
            switch_row(
                "notifications",
                "Desktop notifications",
                "Show a notification for mentions and direct messages while this window is in the background.",
                settings.desktop_notifications,
                toggle_notifications,
            ),
        ))
        .child(group(
            "Layout",
            switch_row(
                "members",
                "Show the member list",
                "Keep the list of people beside server channels.",
                settings.show_members,
                toggle_members,
            ),
        ))
        .child(
            div()
                .text_size(px(12.))
                .text_color(rgb(theme::TEXT_FAINT))
                .child(format!("Oxicord {}", env!("CARGO_PKG_VERSION"))),
        )
        .into_any_element();

    modal(content, close)
}

fn group(title: &'static str, content: impl IntoElement) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .text_size(px(11.))
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(theme::TEXT_MUTED))
                .child(title.to_uppercase()),
        )
        .child(
            div()
                .p_3()
                .rounded(px(8.))
                .bg(rgb(theme::BG_PANEL))
                .child(content),
        )
}

fn switch_row(
    id: &'static str,
    label: &'static str,
    hint: &'static str,
    on: bool,
    handler: Box<Handler>,
) -> impl IntoElement {
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
        .on_click(handler)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(div().text_color(rgb(theme::TEXT_BRIGHT)).child(label))
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(rgb(theme::TEXT_MUTED))
                        .child(hint),
                ),
        )
        .child(track)
}
