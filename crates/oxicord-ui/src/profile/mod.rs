//! Profile cards. `popout` is the small card that opens beside a name, `full`
//! the large one with the board and tabs, and `column` the strip docked
//! beside a direct message. All three draw from the same [`Card`].

mod full;
mod popout;

use std::rc::Rc;

use gpui_kit::{
    AnyElement, App, ClickEvent, Div, Entity, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _,
    Pixels, Point, StatefulInteractiveElement as _, Styled as _, StyledImage as _, Window, div, img, px, rgb, rgba,
    component::input::{Input, InputState},
};
use oxicord_model::Status;

pub use full::full;
pub use popout::popout;

use crate::{
    icons::icon,
    media::{Media, avatar},
    theme,
};

/// Everything the cards show, gathered by the caller so this module doesn't
/// need to know about the store.
pub struct Card {
    pub name: String,
    pub username: String,
    pub bot: bool,
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
    /// The server tag's text and badge picture.
    pub tag: Option<(String, Option<String>)>,
    /// When they joined the server the card was opened from.
    pub joined_server: String,
    /// That server's name and icon address.
    pub server: Option<(String, Option<String>)>,
    pub mutual_servers: Vec<Mutual>,
    pub mutual_friends: Vec<Mutual>,
    /// How many friends we share, which can exceed the list Discord sent.
    pub mutual_friend_count: u32,
    pub accounts: Vec<Connection>,
    /// The game widgets, in the order the board shows them.
    pub board: Vec<Widget>,
    pub activities: Vec<ActivityCard>,
    /// Hide the message button on our own card.
    pub is_me: bool,
    /// Buttons for the person's relationship to us and, in a server, for
    /// moderating them.
    pub actions: Vec<CardAction>,
    /// The private note we keep about them, editable in the card.
    pub note_input: Entity<InputState>,
}

/// A server or a person in common, with a picture and a second line.
pub struct Mutual {
    pub name: String,
    pub picture: Option<String>,
    pub detail: Option<String>,
}

/// An account linked on the profile.
pub struct Connection {
    pub kind: String,
    pub name: String,
    pub since: Option<String>,
    pub games: Option<String>,
}

/// One of the profile's game widgets.
pub struct Widget {
    pub title: &'static str,
    /// The favorite game is drawn large with its comment, the others as a
    /// grid of covers.
    pub featured: bool,
    pub games: Vec<Game>,
}

pub struct Game {
    pub name: String,
    pub cover: Option<String>,
    pub comment: String,
    pub tags: Vec<String>,
}

/// What a person is doing right now, as a card.
pub struct ActivityCard {
    pub header: String,
    pub title: String,
    pub line: String,
    pub image: Option<String>,
    /// Milliseconds played and the length, for something with an end.
    pub progress: Option<(u64, u64)>,
}

pub type Action = Rc<dyn Fn(&mut Window, &mut App)>;
pub type Handler = dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static;

/// A small button under the person's name.
pub struct CardAction {
    pub label: &'static str,
    pub glyph: &'static str,
    pub danger: bool,
    pub run: Action,
}

/// Turns an action into what an element's click wants.
pub fn on_click(action: &Action) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
    let action = action.clone();
    move |_, window, cx| action(window, cx)
}

/// The tabs of the full profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    Board,
    Activity,
    Friends,
    Servers,
}

/// Called with where the button was pressed, to open a menu there.
pub type MenuAction = Rc<dyn Fn(Point<Pixels>, &mut Window, &mut App)>;
/// Called with the tab that was picked.
pub type TabAction = Rc<dyn Fn(Tab, &mut Window, &mut App)>;

/// What the cards can do. The owner builds these from its own state.
pub struct Handlers {
    pub message: Action,
    pub more: MenuAction,
    pub full: Action,
    pub close: Action,
    pub edit: Action,
    pub tab: TabAction,
}

/// The surface of a card laid over the profile, such as a game widget.
pub(crate) const PANEL: u32 = 0x17181b;
/// A faint grey wash, used for chips and quiet buttons.
const WASH: u32 = 0x94949c1f;
/// The outline that goes with it.
const OUTLINE: u32 = 0x9595a229;
/// The thinner outline of tags and role chips.
const OUTLINE_SOFT: u32 = 0x9696a033;

/// A dimmed overlay with `content` centered on it. Clicking outside closes it.
/// `content` should call `.occlude()` so clicks on it stay on it.
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
                .bg(rgba(0x0000_00b8))
                .on_click(on_close),
        )
        .child(content)
        .into_any_element()
}

// ---- shared pieces ----------------------------------------------------

/// A picture in a box, or a flat placeholder while it loads.
pub(crate) fn picture(media: &Entity<Media>, url: &str, width: f32, height: f32, radius: f32, cx: &mut App) -> Div {
    let image = media.update(cx, |m, _| m.get(url));
    let frame = div().w(px(width)).h(px(height)).flex_none().rounded(px(radius)).overflow_hidden();
    match image {
        Some(image) => {
            frame.child(img(gpui_kit::ImageSource::Image(image)).size_full().object_fit(gpui_kit::ObjectFit::Cover))
        }
        None => frame.bg(rgb(theme::CALL_TILE)),
    }
}

/// The banner picture, or a fill of the accent color.
pub(crate) fn banner(card: &Card, media: &Entity<Media>, height: f32, radius: f32, cx: &mut App) -> Div {
    let fill = div().h(px(height)).w_full().flex_none().bg(rgb(card.accent)).overflow_hidden().rounded_t(px(radius));
    match card.banner_url.as_deref().and_then(|url| media.update(cx, |m, _| m.get(url))) {
        Some(image) => {
            fill.child(img(gpui_kit::ImageSource::Image(image)).size_full().object_fit(gpui_kit::ObjectFit::Cover))
        }
        None => fill,
    }
}

/// The avatar with the ring that separates it from the banner and the
/// status mark cut into its corner. `ring_color` is the color behind it.
pub(crate) fn avatar_with_status(card: &Card, media: &Entity<Media>, size: f32, ring: f32, ring_color: u32, cx: &mut App) -> Div {
    let status = (size * 0.2).round();
    let cut = (status * 0.2).round().max(3.);
    let box_size = status + 2. * cut;
    // The status disc is centered on the avatar's lower right at 0.85 of its size.
    let at = size * 0.85 - box_size / 2.;
    div()
        .relative()
        .flex_none()
        .p(px(ring))
        .rounded_full()
        .bg(rgb(ring_color))
        .child(
            div()
                .relative()
                .size(px(size))
                .child(avatar(media, &card.avatar_url, size, &card.name, cx))
                .child(
                    div()
                        .absolute()
                        .left(px(at))
                        .top(px(at))
                        .size(px(box_size))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_full()
                        .bg(rgb(ring_color))
                        .child(crate::member_list::status_shape(card.status, status, ring_color)),
                ),
        )
}

/// The big name line, with the app mark after a bot's name.
pub(crate) fn display_name(card: &Card, size: f32, line: f32) -> Div {
    div()
        .h(px(line))
        .flex()
        .items_center()
        .gap(px(4.))
        .text_size(px(size))
        .line_height(px(line))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(rgb(theme::TEXT))
        .child(card.name.clone())
        .children(card.bot.then(crate::member_list::app_badge))
}

/// The server tag chip: its badge and its letters.
pub(crate) fn tag_chip(tag: &(String, Option<String>), media: &Entity<Media>, cx: &mut App) -> Div {
    div()
        .h(px(24.))
        .flex_none()
        .px(px(8.))
        .flex()
        .items_center()
        .gap(px(4.))
        .rounded(px(8.))
        .bg(rgba(WASH))
        .border_1()
        .border_color(rgba(OUTLINE))
        .text_size(px(12.))
        .line_height(px(16.))
        .text_color(rgb(theme::TEXT))
        .children(tag.1.as_deref().map(|url| picture(media, url, 14., 14., 0., cx)))
        .child(tag.0.clone())
}

/// The row of badges, drawn as one capsule.
pub(crate) fn badge_strip(card: &Card, media: &Entity<Media>, cx: &mut App) -> Option<Div> {
    if card.badges.is_empty() {
        return None;
    }
    let mut strip = div()
        .h(px(22.))
        .flex_none()
        .px(px(3.))
        .flex()
        .items_center()
        .gap(px(2.))
        .rounded(px(8.))
        .border_1()
        .border_color(rgba(OUTLINE_SOFT));
    for (_, url) in &card.badges {
        strip = strip.child(picture(media, url, 20., 20., 0., cx));
    }
    Some(strip)
}

/// A role as a chip with its color dot.
pub(crate) fn role_chip(name: &str, color: Option<u32>) -> Div {
    div()
        .h(px(24.))
        .flex_none()
        .pl(px(8.))
        .pr(px(8.))
        .flex()
        .items_center()
        .gap(px(4.))
        .rounded(px(8.))
        .border_1()
        .border_color(rgba(OUTLINE_SOFT))
        .text_size(px(12.))
        .line_height(px(16.))
        .text_color(rgb(theme::TEXT))
        .child(div().size(px(12.)).flex_none().rounded_full().bg(rgb(color.unwrap_or(theme::OFFLINE))))
        .child(name.to_owned())
}

pub(crate) fn roles_row(card: &Card) -> Div {
    let mut row = div().flex().flex_wrap().gap(px(4.));
    for (name, color) in &card.roles {
        row = row.child(role_chip(name, *color));
    }
    row
}

/// A rounded square picture with the first letters of `name` while it loads.
pub(crate) fn icon_box(media: &Entity<Media>, url: Option<&str>, size: f32, radius: f32, name: &str, cx: &mut App) -> Div {
    let frame = div().size(px(size)).flex_none().rounded(px(radius)).overflow_hidden();
    match url.and_then(|u| media.update(cx, |m, _| m.get(u))) {
        Some(image) => {
            frame.child(img(gpui_kit::ImageSource::Image(image)).size_full().object_fit(gpui_kit::ObjectFit::Cover))
        }
        None => frame
            .flex()
            .items_center()
            .justify_center()
            .bg(rgb(theme::BG_ACTIVE))
            .text_size(px(size * 0.4))
            .font_weight(FontWeight::MEDIUM)
            .text_color(rgb(theme::TEXT_BRIGHT))
            .child(crate::media::acronym(name)),
    }
}

/// How a tag key reads: `looking_for_group` is "Looking for group".
pub(crate) fn tag_label(key: &str) -> String {
    let mut words = key.replace('_', " ");
    if let Some(first) = words.get_mut(0..1) {
        first.make_ascii_uppercase();
    }
    words
}

fn tag_glyph(key: &str) -> &'static str {
    match key {
        "obsessed" => "tag-thumbs-up",
        "better_than_you" => "tag-award",
        _ => "tag-wave",
    }
}

/// One tag under a favorite game.
pub(crate) fn game_tag(key: &str) -> Div {
    div()
        .h(px(20.))
        .flex_none()
        .pl(px(6.))
        .pr(px(8.))
        .flex()
        .items_center()
        .gap(px(5.))
        .rounded(px(8.))
        .border_1()
        .border_color(rgba(OUTLINE_SOFT))
        .text_size(px(10.))
        .line_height(px(12.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(rgb(theme::TEXT_MUTED))
        .child(icon(tag_glyph(key), 12., theme::TEXT_MUTED))
        .child(tag_label(key))
}

/// Minutes and seconds, as the playback clock shows them.
pub(crate) fn clock(milliseconds: u64) -> String {
    let seconds = milliseconds / 1000;
    format!("{:02}:{:02}", seconds / 60, seconds % 60)
}

/// What a current activity looks like: where it is, its picture, its lines
/// and, for something with a length, how far along it is.
pub(crate) fn activity_card(
    activity: &ActivityCard,
    media: &Entity<Media>,
    padding: f32,
    radius: f32,
    image: f32,
    cx: &mut App,
) -> Div {
    let mut details = div().flex_1().min_w_0().flex().flex_col().gap(px(4.)).child(
        div()
            .flex()
            .flex_col()
            .child(
                div()
                    .truncate()
                    .text_size(px(14.))
                    .line_height(px(18.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(theme::TEXT_BRIGHT))
                    .child(activity.title.clone()),
            )
            .children((!activity.line.is_empty()).then(|| {
                div()
                    .truncate()
                    .text_size(px(12.))
                    .line_height(px(16.))
                    .text_color(rgb(theme::TEXT_BRIGHT))
                    .child(activity.line.clone())
            })),
    );
    if let Some((played, length)) = activity.progress {
        let fraction = if length == 0 { 0. } else { (played as f32 / length as f32).clamp(0., 1.) };
        let time = |text: String| {
            div().w(px(36.)).flex_none().text_size(px(12.)).line_height(px(16.)).text_color(rgb(theme::TEXT_BRIGHT)).child(text)
        };
        details = details.child(
            div()
                .h(px(16.))
                .flex()
                .items_center()
                .gap(px(8.))
                .child(time(clock(played.min(length))))
                .child(
                    div()
                        .flex_1()
                        .h(px(2.))
                        .rounded(px(2.))
                        .bg(rgba(0x97979f33))
                        .child(div().h_full().w(gpui_kit::relative(fraction)).rounded(px(2.)).bg(rgb(theme::TEXT_BRIGHT))),
                )
                .child(time(clock(length)).text_right()),
        );
    }
    div()
        .p(px(padding))
        .flex()
        .flex_col()
        .gap(px(8.))
        .rounded(px(radius))
        .bg(rgb(PANEL))
        .child(
            div()
                .h(px(16.))
                .text_size(px(12.))
                .line_height(px(16.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(rgb(theme::TEXT_BRIGHT))
                .child(activity.header.clone()),
        )
        .child(
            div()
                .flex()
                .gap(px(12.))
                .children(activity.image.as_deref().map(|url| picture(media, url, image, image, 8., cx)))
                .child(details),
        )
}

/// A grey section heading as the full profile and its tabs use.
pub(crate) fn heading(text: &str) -> Div {
    div()
        .h(px(16.))
        .text_size(px(12.))
        .line_height(px(16.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(rgb(theme::TEXT_MUTED))
        .child(text.to_owned())
}

/// The bio drawn like a message.
pub(crate) fn bio(card: &Card, media: &Entity<Media>, window: &mut Window, cx: &mut App) -> AnyElement {
    div()
        .text_size(px(14.))
        .line_height(px(18.))
        .text_color(rgb(theme::TEXT_BRIGHT))
        .child(crate::chat::markdown::rich(&card.bio, 0xB10, media, &Default::default(), window, cx))
        .into_any_element()
}

/// The note box with its own heading, as both cards place it.
pub(crate) fn note(card: &Card) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(8.))
        .child(heading("Note (only visible to you)"))
        .child(Input::new(&card.note_input))
}

// ---- the strip beside a direct message ---------------------------------

/// Room the profile column takes beside a conversation.
pub const COLUMN_WIDTH: f32 = 320.;

/// The profile docked beside a direct message: an inset card with the
/// banner on top and the details underneath, as the official client draws it.
pub fn column(card: Card, media: &Entity<Media>, _on_message: Box<Handler>, window: &mut Window, cx: &mut App) -> AnyElement {
    let banner = div().h(px(105.)).w_full().bg(rgb(card.accent)).overflow_hidden();
    let banner = match card.banner_url.as_deref().and_then(|url| media.update(cx, |m, _| m.get(url))) {
        Some(image) => banner.child(img(gpui_kit::ImageSource::Image(image)).size_full().object_fit(gpui_kit::ObjectFit::Cover)),
        None => banner,
    };

    let mut body = div().px(px(16.)).pb(px(16.)).flex().flex_col().child(
        div()
            .h(px(24.))
            .text_size(px(20.))
            .line_height(px(24.))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(rgb(theme::TEXT))
            .child(card.name.clone()),
    );
    body = body.child(
        div()
            .h(px(24.))
            .flex()
            .items_center()
            .gap(px(8.))
            .text_size(px(14.))
            .text_color(rgb(theme::TEXT_BRIGHT))
            .child(card.username.clone())
            .children((!card.pronouns.is_empty()).then(|| {
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(div().size(px(4.)).rounded_full().bg(rgb(theme::TEXT)))
                    .child(div().font_weight(FontWeight::MEDIUM).child(card.pronouns.clone()))
            })),
    );

    if !card.mutual_servers.is_empty() {
        let count = card.mutual_servers.len();
        let mut faces = div().flex().items_center();
        for server in card.mutual_servers.iter().take(3) {
            faces = faces.child(div().mr(px(-4.)).child(avatar(media, server.picture.as_deref().unwrap_or(""), 16., &server.name, cx)));
        }
        body = body.child(
            div()
                .py(px(4.))
                .mt(px(4.))
                .flex()
                .items_center()
                .gap(px(8.))
                .text_size(px(14.))
                .text_color(rgb(theme::TEXT_MUTED))
                .child(faces)
                .child(format!("{count} Mutual Server{}", if count == 1 { "" } else { "s" })),
        );
    }

    if !card.actions.is_empty() {
        let mut row = div().mt(px(8.)).flex().flex_wrap().gap(px(8.));
        for (i, action) in card.actions.iter().enumerate() {
            row = row.child(
                div()
                    .id(("card-action", i as u64))
                    .h(px(32.))
                    .px(px(12.))
                    .flex()
                    .items_center()
                    .rounded(px(theme::RADIUS))
                    .cursor_pointer()
                    .text_size(px(14.))
                    .font_weight(FontWeight::MEDIUM)
                    .bg(rgb(if action.danger { theme::BADGE } else { theme::BG_HOVER }))
                    .text_color(rgb(if action.danger { 0xffffff } else { theme::TEXT_BRIGHT }))
                    .hover(|s| s.opacity(0.85))
                    .on_click(on_click(&action.run))
                    .child(action.label),
            );
        }
        body = body.child(row);
    }

    if !card.bio.is_empty() {
        body = body.child(
            div()
                .mt(px(12.))
                .text_size(px(14.))
                .text_color(rgb(theme::TEXT_BRIGHT))
                .child(crate::chat::markdown::rich(&card.bio, 0xB10, media, &Default::default(), window, cx)),
        );
    }

    let heading = |text: &'static str| {
        div().text_size(px(12.)).line_height(px(16.)).font_weight(FontWeight::SEMIBOLD).text_color(rgb(theme::TEXT_BRIGHT)).child(text)
    };
    let value = |text: String| div().text_size(px(14.)).line_height(px(18.)).text_color(rgb(theme::TEXT)).child(text);
    body = body.child(div().mt(px(20.)).flex().flex_col().gap(px(8.)).child(heading("Created On")).child(value(card.created.clone())));
    if !card.joined_server.is_empty() {
        body = body.child(div().mt(px(16.)).flex().flex_col().gap(px(8.)).child(heading("Member Since")).child(value(card.joined_server.clone())));
    }
    if !card.roles.is_empty() {
        let mut chips = div().flex().flex_wrap().gap(px(4.));
        for (name, color) in &card.roles {
            chips = chips.child(
                div()
                    .h(px(24.))
                    .px(px(8.))
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .rounded(px(8.))
                    .bg(rgb(theme::BG_HOVER))
                    .border_1()
                    .border_color(rgb(theme::BORDER_MUTED))
                    .text_size(px(12.))
                    .child(div().size(px(12.)).rounded_full().bg(rgb(color.unwrap_or(theme::OFFLINE))))
                    .child(name.clone()),
            );
        }
        body = body.child(div().mt(px(16.)).flex().flex_col().gap(px(8.)).child(heading("Roles")).child(chips));
    }
    if !card.accounts.is_empty() {
        let mut list = div().flex().flex_col();
        for account in &card.accounts {
            list = list.child(value(format!("{}: {}", account.kind, account.name)));
        }
        body = body.child(div().mt(px(16.)).flex().flex_col().gap(px(8.)).child(heading("Connections")).child(list));
    }
    if !card.badges.is_empty() {
        let mut row = div().mt(px(12.)).flex().flex_wrap().gap(px(4.));
        for (_, url) in &card.badges {
            if let Some(image) = media.update(cx, |m, _| m.get(url)) {
                row = row.child(img(gpui_kit::ImageSource::Image(image)).size(px(20.)));
            }
        }
        body = body.child(row);
    }

    body = body.child(
        div()
            .mt(px(16.))
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(heading("Note"))
            .child(Input::new(&card.note_input)),
    );

    let header = div()
        .relative()
        .h(px(144.))
        .flex_none()
        .child(banner)
        .child(
            div()
                .absolute()
                .left(px(10.))
                .top(px(51.))
                .p(px(6.))
                .rounded_full()
                .bg(rgb(theme::BG_INPUT))
                .child(
                    div()
                        .relative()
                        .child(avatar(media, &card.avatar_url, 80., &card.name, cx))
                        .child(
                            div()
                                .absolute()
                                .right(px(-4.))
                                .bottom(px(-4.))
                                .p(px(3.))
                                .rounded_full()
                                .bg(rgb(theme::BG_INPUT))
                                .child(crate::member_list::status_shape(card.status, 14., theme::BG_INPUT)),
                        ),
                ),
        );

    div()
        .id("profile-column")
        .occlude()
        .w(px(COLUMN_WIDTH))
        .h_full()
        .flex_none()
        .p(px(8.))
        .child(
            div()
                .id("profile-column-card")
                .size_full()
                .rounded(px(theme::RADIUS))
                .bg(rgb(theme::BG_INPUT))
                .overflow_y_scroll()
                .child(header)
                .child(body),
        )
        .into_any_element()
}
