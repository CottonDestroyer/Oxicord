//! The small card that opens beside a name or an avatar.

use gpui_kit::{
    AnyElement, Entity, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _, Pixels, Point,
    StatefulInteractiveElement as _, Styled as _, Window, anchored, div, img, point, px, rgb, rgba,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::component::input::Input;

use super::{
    Card, Handlers, PANEL, Tab, activity_card, avatar_with_status, badge_strip, banner, bio, display_name, icon_box,
    on_click, picture, roles_row, tag_chip,
};
use crate::{icons::icon, media::{Media, avatar}, theme};

const WIDTH: f32 = 300.;
/// Gap kept between the card and the window edge.
const MARGIN: f32 = 16.;
/// A bio taller than this many lines is cut and offers the full profile.
const BIO_LINES: usize = 3;

/// Where the card's top left wants to go. Next to the member list it opens
/// on that list's left, elsewhere beside the click, and without a click in
/// the corner under the header. The card is slid up or left afterwards if its
/// height or width would run past the window.
fn place(anchor: Option<(f32, f32)>, width: f32) -> (f32, f32) {
    let Some((ax, ay)) = anchor else { return (width - WIDTH - MARGIN, 56.) };
    let list_left = width - theme::MEMBERS_WIDTH;
    // A click at the middle of a 40px avatar puts the card 8px from its edge.
    let mut x = if ax >= list_left { list_left - 8. - WIDTH } else { ax + 28. };
    if x + WIDTH > width - MARGIN {
        x = ax - 28. - WIDTH;
    }
    let x = x.clamp(MARGIN, (width - WIDTH - MARGIN).max(MARGIN));
    (x, (ay - 20.).max(MARGIN))
}

/// The thought bubble of a custom status, with its two trailing dots.
fn status_bubble(card: &Card, handlers: &Handlers) -> gpui_kit::Stateful<gpui_kit::Div> {
    let fill = 0x1a1a1c;
    let dot = |left: f32, top: f32, size: f32| {
        div().absolute().left(px(left)).top(px(top)).size(px(size)).rounded_full().bg(rgb(fill))
    };
    let (text, set) = match &card.custom_status {
        Some(text) => (text.clone(), false),
        None => ("Most satisfying game mechanic?".to_owned(), true),
    };
    let mut bubble = div()
        .id("popout-status")
        .absolute()
        .left(px(109.))
        .top(px(98.))
        .w(px(179.))
        .min_h(px(36.))
        .px(px(12.))
        .py(px(8.))
        .flex()
        .items_start()
        .gap(px(6.))
        .rounded(px(16.))
        .bg(rgb(fill))
        .text_size(px(14.))
        .line_height(px(18.))
        .italic()
        .text_color(rgb(if set { theme::TEXT_FAINT } else { theme::TEXT }))
        .child(dot(-14., -8., 6.))
        .child(dot(-6., -14., 10.));
    if set {
        bubble = bubble
            .cursor_pointer()
            .on_click(on_click(&handlers.status))
            .child(div().mt(px(1.)).child(icon("circle-plus", 16., theme::TEXT_FAINT)));
    }
    bubble.child(div().flex_1().min_w_0().child(text))
}

fn long_bio(bio: &str) -> bool {
    bio.lines().count() > BIO_LINES || bio.chars().count() > BIO_LINES * 40
}

/// The card, over a clear layer that closes it when clicked.
pub fn popout(
    card: Card,
    handlers: &Handlers,
    anchor: Option<Point<Pixels>>,
    media: &Entity<Media>,
    window: &mut Window,
    cx: &mut gpui_kit::App,
) -> AnyElement {
    let viewport = window.viewport_size();
    let (width, height) = (f32::from(viewport.width), f32::from(viewport.height));
    let (x, y) = place(anchor.map(|p| (f32::from(p.x), f32::from(p.y))), width);

    // The frame's layers, split by depth so the back ones sit behind the card.
    let (mut frame_back, mut frame_front) = (Vec::new(), Vec::new());
    // Discord hides the frame when the window is too short to fit it.
    if height >= 480. {
        if let Some(frame) = &card.frame {
        let scale = WIDTH / frame.inner_width as f32;
        let soh = frame.overflow_horizontal as f32 * scale;
        let sot = frame.overflow_top as f32 * scale;
        let sob = frame.overflow_bottom as f32 * scale;
        let frame_width = WIDTH + 2.0 * soh;
        for layer in &frame.layers {
            if layer.kind != "staple" {
                continue;
            }
            let url = format!(
                "https://cdn.discordapp.com/media/v1/collectibles-shop/{}/{}",
                frame.sku, layer.id
            );
            let url = format!("{}/static", url);
            let Some(image) = media.update(cx, |m, _| m.get(&url)) else {
                continue;
            };
            let mut el = div()
                .absolute()
                .left(px(-soh))
                .w(px(frame_width))
                .child(img(gpui_kit::ImageSource::Image(image)).w(px(frame_width)));
            el = match layer.anchor.as_str() {
                "top" => el.top(px(-sot)),
                "bottom" => el.bottom(px(-sob)),
                _ => el.top_0(),
            };
            if layer.order == "back" {
                frame_back.push(el);
            } else {
                frame_front.push(el);
            }
        }
    }
    }
    
    let add = card.actions.iter().find(|a| matches!(a.label, "Add Friend" | "Accept")).map(|a| a.run.clone());
    let round_button = |id: &'static str, glyph: &'static str| {
        div()
            .id(id)
            .size(px(32.))
            .flex()
            .items_center()
            .justify_center()
            .rounded_full()
            .bg(rgba(0x0000_008f))
            .cursor_pointer()
            .hover(|s| s.bg(rgba(0x0000_00b3)))
            .child(icon(glyph, 16., theme::TEXT_BRIGHT))
    };
    let more = handlers.more.clone();
    // A custom status hangs beside the avatar as a thought bubble, which is
    // also where we are asked to set one.
    let bubble = (card.custom_status.is_some() || card.is_me).then(|| status_bubble(&card, handlers));
    let header = div()
        .relative()
        .h(px(if bubble.is_some() { 152. } else { 140. }))
        .flex_none()
        .child(banner(&card, media, 105., 8., cx))
        .child(
            div()
                .id("popout-avatar")
                .absolute()
                .left(px(10.))
                .top(px(55.))
                .cursor_pointer()
                .on_click(on_click(&handlers.full))
                .child(avatar_with_status(&card, media, 80., 6., theme::BG_PANEL, cx)),
        )
        .children(bubble)
        .child(
            div()
                .absolute()
                .top(px(8.))
                .right(px(12.))
                .flex()
                .gap(px(8.))
                .children(add.map(|add| round_button("popout-add", "friend-add").on_click(on_click(&add))))
                .child(round_button("popout-more", "more-dots").on_click(move |event, window, cx| {
                    more(event.position(), window, cx)
                })),
        );

    // Name, username, pronouns, server tag and badges wrap together.
    let line = |text: String| {
        div().h(px(24.)).flex().items_center().text_size(px(14.)).line_height(px(18.)).text_color(rgb(theme::TEXT_BRIGHT)).child(text)
    };
    let mut who = div().flex().flex_wrap().items_center().gap_x(px(6.)).gap_y(px(2.)).child(line(card.username.clone()));
    if !card.pronouns.is_empty() {
        who = who
            .child(div().size(px(4.)).flex_none().rounded_full().bg(rgb(theme::TEXT)))
            .child(line(card.pronouns.clone()));
    }
    if let Some(tag) = &card.tag {
        who = who.child(tag_chip(tag, media, cx));
    }
    who = who.children(badge_strip(&card, media, 6, cx));

    let mut body = div()
        .id("popout-body")
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .pt(px(4.))
        .pr(px(8.))
        .pb(px(8.))
        .pl(px(16.))
        .flex()
        .flex_col()
        .gap(px(12.))
        .child(
            div().flex().flex_col().child(
                div()
                    .id("popout-name")
                    .cursor_pointer()
                    .on_click(on_click(&handlers.full))
                    .child(display_name(&card, 20., 24.)),
            ).child(who),
        );

    // People and servers in common, each opening its tab.
    let mut mutuals = div().flex().flex_wrap().gap_x(px(6.)).gap_y(px(2.));
    if !card.mutual_servers.is_empty() {
        let count = card.mutual_servers.len();
        let mut faces = div().flex().items_center();
        for (i, server) in card.mutual_servers.iter().take(3).enumerate() {
            faces = faces.child(
                div().ml(px(if i == 0 { 0. } else { -3. })).child(icon_box(
                    media,
                    server.picture.as_deref(),
                    16.,
                    4.,
                    &server.name,
                    cx,
                )),
            );
        }
        let open = handlers.tab.clone();
        mutuals = mutuals.child(
            div()
                .id("popout-mutual-servers")
                .py(px(4.))
                .flex()
                .items_center()
                .gap(px(4.))
                .cursor_pointer()
                .text_size(px(14.))
                .line_height(px(18.))
                .text_color(rgb(theme::TEXT_MUTED))
                .on_click(move |_, window, cx| open(Tab::Servers, window, cx))
                .child(faces)
                .child(format!("{count} Mutual Server{}", if count == 1 { "" } else { "s" })),
        );
    }
    if card.mutual_friend_count > 0 {
        let count = card.mutual_friend_count;
        let mut faces = div().flex().items_center();
        for (i, friend) in card.mutual_friends.iter().take(3).enumerate() {
            faces = faces.child(
                div()
                    .ml(px(if i == 0 { 0. } else { -3. }))
                    .child(avatar(media, friend.picture.as_deref().unwrap_or(""), 16., &friend.name, cx)),
            );
        }
        let open = handlers.tab.clone();
        mutuals = mutuals.child(
            div()
                .id("popout-mutual-friends")
                .py(px(4.))
                .flex()
                .items_center()
                .gap(px(4.))
                .cursor_pointer()
                .text_size(px(14.))
                .line_height(px(18.))
                .text_color(rgb(theme::TEXT_MUTED))
                .on_click(move |_, window, cx| open(Tab::Friends, window, cx))
                .child(faces)
                .child(format!("{count} Mutual Friend{}", if count == 1 { "" } else { "s" })),
        );
    }
    if !card.mutual_servers.is_empty() || card.mutual_friend_count > 0 {
        body = body.child(mutuals);
    }

    if !card.bio.is_empty() {
        let long = long_bio(&card.bio);
        let text = bio(&card, media, window, cx);
        body = body.child(
            div()
                .flex()
                .flex_col()
                .gap(px(4.))
                .child(div().when(long, |d| d.max_h(px(54.)).overflow_hidden()).child(text))
                .children(long.then(|| {
                    div()
                        .id("popout-view-bio")
                        .h(px(16.))
                        .cursor_pointer()
                        .text_size(px(12.))
                        .line_height(px(16.))
                        .text_color(rgb(theme::TEXT_BRIGHT))
                        .hover(|s| s.text_color(rgb(0xffffff)))
                        .on_click(on_click(&handlers.full))
                        .child("View Full Bio")
                })),
        );
    }

    // The board in brief: a strip of the covers, opening the full profile.
    let games: Vec<&str> = {
        let mut seen: Vec<&str> = Vec::new();
        for game in card.board.iter().flat_map(|w| &w.games) {
            if let Some(cover) = game.cover.as_deref()
                && !seen.contains(&cover)
            {
                seen.push(cover);
            }
        }
        seen
    };
    if !games.is_empty() {
        let total = games.len();
        let shown = if total > 4 { 3 } else { total };
        let mut covers = div().flex().gap(px(4.));
        for url in games.iter().take(shown) {
            covers = covers.child(picture(media, url, 24., 24., 4., cx));
        }
        if total > 4 {
            covers = covers.child(
                div()
                    .relative()
                    .child(picture(media, games[3], 24., 24., 4., cx))
                    .child(
                        div()
                            .absolute()
                            .top_0()
                            .left_0()
                            .size(px(24.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(4.))
                            .bg(rgba(0x0000_0099))
                            .text_size(px(12.))
                            .line_height(px(16.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgb(0xe0e0e3))
                            .child(format!("+{}", total - 3)),
                    ),
            );
        }
        body = body.child(
            div()
                .id("popout-collection")
                .h(px(40.))
                .p(px(8.))
                .flex()
                .items_center()
                .justify_between()
                .rounded(px(8.))
                .bg(rgb(PANEL))
                .cursor_pointer()
                .on_click(on_click(&handlers.full))
                .child(
                    div()
                        .text_size(px(12.))
                        .line_height(px(16.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(rgb(theme::TEXT_BRIGHT))
                        .child("Game Collection"),
                )
                .child(covers),
        );
    }

    for activity in &card.activities {
        body = body.child(activity_card(activity, media, 8., 8., 60., cx));
    }

    if !card.roles.is_empty() {
        body = body.child(roles_row(&card));
    }

    // The footer: a message box that opens the conversation, or for our own
    // card a way to edit the profile.
    let footer = if card.is_me {
        div()
            .id("popout-edit")
            .h(px(32.))
            .flex()
            .items_center()
            .justify_center()
            .gap(px(4.))
            .rounded(px(8.))
            .bg(rgb(theme::ACCENT))
            .cursor_pointer()
            .hover(|s| s.opacity(0.85))
            .text_size(px(14.))
            .line_height(px(18.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(rgb(0xffffff))
            .on_click(on_click(&handlers.edit))
            .child(icon("pencil", 16., 0xffffff))
            .child("Edit Profile")
    } else {
        let typed = !card.message_input.read(cx).value().is_empty();
        div()
            .id("popout-message")
            .relative()
            .h(px(46.))
            .pl(px(15.))
            .pr(px(8.))
            .flex()
            .items_center()
            .rounded(px(8.))
            .bg(rgb(0x09090b))
            .border_1()
            .border_color(rgb(theme::BORDER_MUTED))
            .cursor_text()
            // The box has no placeholder of its own that can name the person.
            .children((!typed).then(|| {
                div()
                    .absolute()
                    .left(px(15.))
                    .top_0()
                    .bottom_0()
                    .flex()
                    .items_center()
                    .text_size(px(14.))
                    .line_height(px(22.))
                    .text_color(rgb(theme::TEXT_FAINT))
                    .child(format!("Message @{}", card.name))
            }))
            .child(div().flex_1().min_w_0().child(Input::new(&card.message_input).appearance(false).bordered(false).focus_bordered(false)))
            .child(div().size(px(32.)).flex_none().flex().items_center().justify_center().child(icon(
                "face-slightly-smiling",
                20.,
                theme::TEXT_MUTED,
            )))
    };

    div()
        .id("popout-overlay")
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .child(
            div()
                .id("popout-scrim")
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .on_click(on_click(&handlers.close)),
        )
        .child(
            anchored().position(point(px(x), px(y))).snap_to_window_with_margin(px(MARGIN)).child(
            div()
                .relative()
                .children(frame_back)
                .child(
                    div()
                        .id("popout-card")
                        .occlude()
                        .w(px(WIDTH))
                        .max_h(px((height - 2. * MARGIN).max(240.)))
                        .flex()
                        .flex_col()
                        .gap(px(8.))
                        .rounded(px(8.))
                        .bg(rgb(theme::BG_PANEL))
                        .border_1()
                        .border_color(rgba(0xffffff14))
                        .shadow_lg()
                        .overflow_hidden()
                        // The effect sits over the card's fill but under its words.
                        .children(card.effect.clone())
                        .child(header)
                        .child(body)
                        .child(div().flex_none().px(px(16.)).pb(px(12.)).child(footer)),
                )
                .children(frame_front),
            ),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn beside_the_member_list_the_card_opens_on_its_left() {
        // The member list starts at 1136 in a 1400 wide window.
        let (x, y) = place(Some((1250., 146.)), 1400.);
        assert_eq!((x, y), (828., 126.));
    }

    #[test]
    fn a_click_in_the_chat_opens_the_card_to_its_right_and_flips_at_the_edge() {
        let (x, _) = place(Some((500., 300.)), 1400.);
        assert_eq!(x, 528.);
        // Not enough room on the right, so it goes to the left of the click.
        let (x, _) = place(Some((1100., 300.)), 1400.);
        assert_eq!(x, 1100. - 28. - WIDTH);
    }

    #[test]
    fn the_card_stays_inside_the_window() {
        let (x, y) = place(Some((10., 2.)), 800.);
        assert!(x >= MARGIN && y >= MARGIN);
        assert_eq!(place(None, 1000.), (684., 56.));
    }

    #[test]
    fn only_a_long_bio_is_cut() {
        assert!(!long_bio("short"));
        assert!(long_bio("a\nb\nc\nd"));
        assert!(long_bio(&"x".repeat(121)));
    }
}
