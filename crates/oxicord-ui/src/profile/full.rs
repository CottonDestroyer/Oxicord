//! The full profile: the person on the left, the board and tabs on the right.

use gpui_kit::{
    AnyElement, Div, Entity, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, Window, div, px, rgb, rgba,
};

use super::{
    Card, Connection, Handlers, OUTLINE, OUTLINE_SOFT, PANEL, Tab, WASH, Widget, activity_card, avatar_with_status,
    badge_strip, banner, bio, display_name, game_tag, heading, icon_box, modal, note, on_click, picture, roles_row,
    tag_chip,
};
use crate::{icons::icon, media::Media, theme};

const WIDTH: f32 = 960.;
/// Room left above and below the window.
const MARGIN: f32 = 51.;
/// The cover of a game on the board.
const COVER: (f32, f32) = (87., 116.);

/// The tabs a person has, in order, with their labels. The board is there
/// only when they filled one in.
fn tab_list(board: bool, friends: usize, servers: usize) -> Vec<(Tab, String)> {
    let plural = |n: usize, one: &str, many: &str| match n {
        0 => format!("No {many}"),
        1 => format!("1 {one}"),
        n => format!("{n} {many}"),
    };
    let mut out = Vec::new();
    if board {
        out.push((Tab::Board, "Board".to_owned()));
    }
    out.push((Tab::Activity, "Activity".to_owned()));
    out.push((Tab::Friends, plural(friends, "Mutual Friend", "Mutual Friends")));
    out.push((Tab::Servers, plural(servers, "Mutual Server", "Mutual Servers")));
    out
}

fn tabs(card: &Card) -> Vec<(Tab, String)> {
    tab_list(!card.board.is_empty(), card.mutual_friend_count as usize, card.mutual_servers.len())
}

/// The tab to show: the one asked for if this person has it, else the first.
pub(super) fn visible_tab(card: &Card, wanted: Tab) -> Tab {
    let all = tabs(card);
    if all.iter().any(|(tab, _)| *tab == wanted) { wanted } else { all[0].0 }
}

pub fn full(
    card: Card,
    handlers: &Handlers,
    tab: Tab,
    media: &Entity<Media>,
    window: &mut Window,
    cx: &mut gpui_kit::App,
) -> AnyElement {
    let viewport = window.viewport_size();
    let (width, height) = (f32::from(viewport.width), f32::from(viewport.height));
    let tab = visible_tab(&card, tab);

    let left = left_column(&card, handlers, media, window, cx);
    let right = right_column(&card, handlers, tab, media, cx);

    let content = div()
        .id("profile-full")
        .occlude()
        .relative()
        .w(px(WIDTH.min(width - 32.)))
        .h(px((height - 2. * MARGIN).clamp(360., 840.)))
        .flex_none()
        .flex()
        .pt(px(48.))
        .pr(px(32.))
        .pl(px(48.))
        .rounded(px(16.))
        .overflow_hidden()
        .bg(rgb(0x000000))
        .border_1()
        .border_color(rgb(theme::BORDER))
        .child(left)
        .child(right)
        .child(
            div()
                .id("profile-close")
                .absolute()
                .top(px(7.))
                .right(px(7.))
                .size(px(32.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(8.))
                .bg(rgba(WASH))
                .border_1()
                .border_color(rgba(OUTLINE))
                .cursor_pointer()
                .hover(|s| s.opacity(0.8))
                .on_click(on_click(&handlers.close))
                .child(icon("x", 16., theme::TEXT_BRIGHT)),
        )
        .into_any_element();

    let close = handlers.close.clone();
    modal(content, Box::new(move |_, window, cx| close(window, cx)))
}

// ---- left: the person ------------------------------------------------------

fn button(id: &'static str) -> gpui_kit::Stateful<Div> {
    div().id(id).h(px(32.)).flex().items_center().justify_center().rounded(px(8.)).cursor_pointer()
}

/// A square wash button with an icon.
fn icon_button(id: &'static str, glyph: &'static str) -> gpui_kit::Stateful<Div> {
    button(id)
        .w(px(32.))
        .flex_none()
        .bg(rgba(WASH))
        .border_1()
        .border_color(rgba(OUTLINE))
        .hover(|s| s.opacity(0.8))
        .child(icon(glyph, 16., theme::TEXT_BRIGHT))
}

fn primary(id: &'static str, glyph: &'static str, label: &str) -> gpui_kit::Stateful<Div> {
    button(id)
        .px(px(11.))
        .gap(px(4.))
        .bg(rgb(theme::ACCENT))
        .border_1()
        .border_color(rgba(0xffffff14))
        .hover(|s| s.opacity(0.85))
        .text_size(px(14.))
        .line_height(px(18.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(rgb(0xffffff))
        .child(icon(glyph, 16., 0xffffff))
        .child(label.to_owned())
}

fn section(title: &str) -> Div {
    div().flex().flex_col().gap(px(8.)).child(heading(title))
}

fn connection(account: &Connection) -> Div {
    div()
        .flex()
        .gap(px(8.))
        .child(
            div()
                .size(px(24.))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(8.))
                .bg(rgb(0xffffff))
                .border_1()
                .border_color(rgba(OUTLINE_SOFT))
                .child(icon("link", 14., 0x1e1f22)),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(4.))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(2.))
                                .text_size(px(14.))
                                .line_height(px(18.))
                                .text_color(rgb(theme::TEXT))
                                .child(div().truncate().child(account.name.clone()))
                                .child(icon("link-out", 16., theme::TEXT)),
                        )
                        .children(account.since.as_ref().map(|since| {
                            div()
                                .text_size(px(12.))
                                .line_height(px(16.))
                                .text_color(rgb(theme::TEXT))
                                .child(format!("Member since {since}"))
                        })),
                )
                .children(account.games.as_ref().map(|games| {
                    div()
                        .flex()
                        .gap(px(3.))
                        .text_size(px(10.))
                        .line_height(px(12.))
                        .text_color(rgb(theme::TEXT))
                        .child("Games")
                        .child(div().font_weight(FontWeight::BOLD).child(games.clone()))
                })),
        )
}

fn left_column(card: &Card, handlers: &Handlers, media: &Entity<Media>, window: &mut Window, cx: &mut gpui_kit::App) -> Div {
    let more = handlers.more.clone();

    let mut who = div().flex().flex_wrap().items_center().gap_x(px(6.)).gap_y(px(4.));
    let line = |text: String| {
        div().h(px(24.)).flex().items_center().text_size(px(14.)).line_height(px(18.)).text_color(rgb(theme::TEXT_BRIGHT)).child(text)
    };
    who = who.child(line(card.username.clone()));
    if !card.pronouns.is_empty() {
        who = who
            .child(div().size(px(4.)).flex_none().rounded_full().bg(rgb(theme::TEXT)))
            .child(line(card.pronouns.clone()));
    }
    if let Some(tag) = &card.tag {
        who = who.child(tag_chip(tag, media, cx));
    }
    who = who.children(badge_strip(card, media, cx));

    // Our own profile gets Edit Profile, anyone else the friend button, the
    // message button and a menu.
    let relation = card.actions.iter().find(|a| matches!(a.label, "Add Friend" | "Accept" | "Unblock"));
    let mut buttons = div().flex().gap(px(8.));
    if card.is_me {
        buttons = buttons.child(primary("full-edit", "pencil", "Edit Profile").on_click(on_click(&handlers.edit)));
    } else if let Some(action) = relation {
        let glyph = if action.label == "Unblock" { "ban" } else { "friend-add" };
        buttons = buttons
            .child(primary("full-relation", glyph, action.label).on_click(on_click(&action.run)))
            .child(icon_button("full-message", "message-bubble").on_click(on_click(&handlers.message)));
    } else {
        buttons = buttons.child(primary("full-send", "message-bubble", "Message").on_click(on_click(&handlers.message)));
    }
    buttons = buttons.child(
        icon_button("full-more", "more-dots").on_click(move |event, window, cx| more(event.position(), window, cx)),
    );

    let mut body = div()
        .id("profile-left-body")
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .pt(px(4.))
        .pr(px(24.))
        .pb(px(32.))
        .pl(px(32.))
        .flex()
        .flex_col()
        .gap(px(24.))
        .child(div().flex().flex_col().child(display_name(card, 24., 32.)).child(who))
        .child(buttons);

    if !card.bio.is_empty() {
        body = body.child(bio(card, media, window, cx));
    }

    let mut since = div().flex().items_center().gap(px(8.)).text_size(px(14.)).line_height(px(18.)).text_color(rgb(theme::TEXT));
    since = since.child(
        div()
            .flex()
            .items_center()
            .gap(px(4.))
            .child(icon("discord-mark", 16., theme::TEXT_MUTED))
            .child(card.created.clone()),
    );
    if !card.joined_server.is_empty() {
        let (name, picture) = card.server.clone().unwrap_or_default();
        since = since.child(div().size(px(4.)).flex_none().rounded_full().bg(rgb(theme::TEXT_MUTED))).child(
            div()
                .flex()
                .items_center()
                .gap(px(4.))
                .child(icon_box(media, picture.as_deref(), 16., 4., &name, cx))
                .child(card.joined_server.clone()),
        );
    }
    body = body.child(section("Member Since").child(since));

    if !card.roles.is_empty() {
        body = body.child(section("Roles").gap(px(6.)).child(roles_row(card)));
    }

    if !card.accounts.is_empty() {
        let mut list = div().flex().flex_col().gap(px(16.));
        for account in &card.accounts {
            list = list.child(connection(account));
        }
        body = body.child(section("Connections").child(list));
    }

    body = body.child(note(card));

    div()
        .w(px(400.))
        .h_full()
        .flex_none()
        .flex()
        .flex_col()
        .rounded_t(px(16.))
        .overflow_hidden()
        .bg(rgb(theme::BG_PANEL))
        .child(
            div()
                .relative()
                .h(px(216.))
                .flex_none()
                .child(banner(card, media, 140., 16., cx))
                .child(
                    div()
                        .absolute()
                        .left(px(24.))
                        .top(px(72.))
                        .child(avatar_with_status(card, media, 120., 8., theme::BG_PANEL, cx)),
                ),
        )
        .child(body)
}

// ---- right: tabs --------------------------------------------------------

fn right_column(card: &Card, handlers: &Handlers, tab: Tab, media: &Entity<Media>, cx: &mut gpui_kit::App) -> Div {
    let mut bar = div().h(px(30.)).flex().gap(px(24.)).border_b_1().border_color(rgba(OUTLINE_SOFT));
    for (index, (this, label)) in tabs(card).into_iter().enumerate() {
        let selected = this == tab;
        let open = handlers.tab.clone();
        bar = bar.child(
            div()
                .id(("profile-tab", index as u64))
                .relative()
                .pb(px(16.))
                .cursor_pointer()
                .text_size(px(14.))
                .line_height(px(14.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(if selected { theme::TEXT_BRIGHT } else { theme::TEXT_MUTED }))
                .hover(|s| s.text_color(rgb(theme::TEXT_BRIGHT)))
                .on_click(move |_, window, cx| open(this, window, cx))
                .child(label)
                .children(selected.then(|| {
                    div().absolute().left_0().right_0().bottom(px(-1.)).h(px(2.)).rounded(px(2.)).bg(rgb(theme::TEXT_BRIGHT))
                })),
        );
    }

    let panel = match tab {
        Tab::Board => board(card, media, cx),
        Tab::Activity => activity(card, media, cx),
        Tab::Friends => people(card, media, cx),
        Tab::Servers => servers(card, media, cx),
    };

    div()
        .flex_1()
        .min_w_0()
        .h_full()
        .flex()
        .flex_col()
        .pt(px(16.))
        .pl(px(36.))
        .pr(px(16.))
        .child(bar)
        .child(
            div()
                .id(("profile-panel", tab as u64))
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .pt(px(20.))
                .pb(px(16.))
                .child(panel),
        )
}

fn board(card: &Card, media: &Entity<Media>, cx: &mut gpui_kit::App) -> Div {
    let mut column = div().flex().flex_col().gap(px(12.));
    for widget in &card.board {
        column = column.child(widget_card(widget, media, cx));
    }
    column
}

fn widget_card(widget: &Widget, media: &Entity<Media>, cx: &mut gpui_kit::App) -> Div {
    let mut body = div().flex().flex_col().gap(px(8.)).child(
        div()
            .h(px(24.))
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .text_size(px(14.))
                    .line_height(px(18.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(theme::TEXT))
                    .child(widget.title),
            )
            .child(icon("more-dots", 18., theme::TEXT_MUTED)),
    );

    if widget.featured {
        for game in &widget.games {
            let cover = game.cover.as_deref().map_or_else(
                || div().w(px(COVER.0)).h(px(COVER.1)).flex_none().rounded(px(8.)).bg(rgb(theme::CALL_TILE)),
                |url| picture(media, url, COVER.0, COVER.1, 8., cx),
            );
            let mut details = div().flex_1().min_w_0().pl(px(7.)).flex().flex_col().gap(px(4.)).child(
                div()
                    .text_size(px(14.))
                    .line_height(px(18.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(theme::TEXT))
                    .child(game.name.clone()),
            );
            if !game.comment.is_empty() {
                details = details.child(
                    div()
                        .flex()
                        .items_start()
                        .gap(px(4.))
                        .text_size(px(14.))
                        .line_height(px(18.))
                        .text_color(rgb(theme::TEXT_FAINT))
                        .child(div().mt(px(3.)).child(icon("quote-mark", 12., theme::TEXT_FAINT)))
                        .child(div().flex_1().min_w_0().line_clamp(4).child(game.comment.clone())),
                );
            }
            if !game.tags.is_empty() {
                let mut tags = div().flex().flex_wrap().gap(px(4.));
                for tag in &game.tags {
                    tags = tags.child(game_tag(tag));
                }
                details = details.child(tags);
            }
            body = body.child(div().flex().gap(px(16.)).child(cover).child(details));
        }
    } else {
        let mut grid = div().flex().flex_wrap().gap(px(16.));
        for game in &widget.games {
            grid = grid.child(game.cover.as_deref().map_or_else(
                || {
                    div()
                        .w(px(COVER.0))
                        .h(px(COVER.1))
                        .flex_none()
                        .rounded(px(8.))
                        .bg(rgb(theme::CALL_TILE))
                        .p(px(6.))
                        .text_size(px(12.))
                        .text_color(rgb(theme::TEXT_MUTED))
                        .child(game.name.clone())
                },
                |url| picture(media, url, COVER.0, COVER.1, 8., cx),
            ));
        }
        body = body.child(grid);
    }

    div()
        .pt(px(12.))
        .px(px(16.))
        .pb(px(16.))
        .rounded(px(16.))
        .bg(rgb(PANEL))
        .border_1()
        .border_color(rgba(OUTLINE))
        .child(body)
}

fn activity(card: &Card, media: &Entity<Media>, cx: &mut gpui_kit::App) -> Div {
    if card.activities.is_empty() {
        return empty("Nothing to show right now", "When they're up to something, you'll see it here.");
    }
    let mut cards = div().flex().flex_col().gap(px(8.));
    for activity in &card.activities {
        cards = cards.child(activity_card(activity, media, 12., 12., 100., cx));
    }
    div().flex().flex_col().gap(px(8.)).child(heading("Current activity")).child(cards)
}

fn row(media: &Entity<Media>, picture: Option<&str>, radius: f32, name: &str, detail: Option<&str>, cx: &mut gpui_kit::App) -> Div {
    div()
        .h(px(50.))
        .p(px(4.))
        .flex()
        .items_center()
        .gap(px(10.))
        .rounded(px(4.))
        .child(icon_box(media, picture, 40., radius, name, cx))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(
                    div()
                        .truncate()
                        .text_size(px(16.))
                        .line_height(px(20.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(rgb(theme::TEXT))
                        .child(name.to_owned()),
                )
                .children(detail.map(|detail| {
                    div()
                        .truncate()
                        .text_size(px(12.))
                        .line_height(px(16.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(rgb(theme::TEXT))
                        .child(detail.to_owned())
                })),
        )
}

fn servers(card: &Card, media: &Entity<Media>, cx: &mut gpui_kit::App) -> Div {
    if card.mutual_servers.is_empty() {
        return empty("You don't share any servers", "Join the same servers to see them here.");
    }
    let mut list = div().flex().flex_col().gap(px(12.));
    for server in &card.mutual_servers {
        list = list.child(row(media, server.picture.as_deref(), 12., &server.name, server.detail.as_deref(), cx));
    }
    list
}

fn people(card: &Card, media: &Entity<Media>, cx: &mut gpui_kit::App) -> Div {
    if card.mutual_friends.is_empty() {
        return empty(
            "You don't have any friends in common",
            "Join the same servers to meet more mutual friends.",
        );
    }
    let mut list = div().flex().flex_col().gap(px(12.));
    for friend in &card.mutual_friends {
        list = list.child(row(media, friend.picture.as_deref(), 20., &friend.name, friend.detail.as_deref(), cx));
    }
    list
}

/// The message a tab shows when there is nothing to list.
fn empty(title: &str, text: &str) -> Div {
    div().h(px(560.)).flex().items_center().justify_center().child(
        div()
            .w(px(320.))
            .flex()
            .flex_col()
            .items_center()
            .gap(px(8.))
            .text_center()
            .child(
                div()
                    .text_size(px(16.))
                    .line_height(px(20.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(theme::TEXT_BRIGHT))
                    .child(title.to_owned()),
            )
            .child(div().text_size(px(14.)).line_height(px(18.)).text_color(rgb(theme::TEXT)).child(text.to_owned())),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tag_keys_read_as_words() {
        assert_eq!(super::super::tag_label("looking_for_group"), "Looking for group");
        assert_eq!(super::super::tag_label("obsessed"), "Obsessed");
    }

    #[test]
    fn the_board_tab_only_exists_for_a_filled_board() {
        let names = |board, friends, servers| -> Vec<String> {
            tab_list(board, friends, servers).into_iter().map(|(_, label)| label).collect()
        };
        assert_eq!(names(false, 0, 6), ["Activity", "No Mutual Friends", "6 Mutual Servers"]);
        assert_eq!(names(true, 1, 1), ["Board", "Activity", "1 Mutual Friend", "1 Mutual Server"]);
    }
}
