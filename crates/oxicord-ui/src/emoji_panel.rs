//! The emoji picker: a search box, category tabs and a grid. It is shared by
//! the composer and by reactions, the owner decides what a pick does.

use emojis::Group;
use gpui_kit::{
    AppContext as _, Context, Entity, EventEmitter, FontWeight, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, SharedString, StatefulInteractiveElement as _,
    Styled as _, StyledImage as _, Subscription, Window, div, img, px, rgb,
    component::input::{Input, InputEvent, InputState},
};
use oxicord_media::cdn;
use oxicord_model::{Emoji, Gif, Snowflake, StickerItem};

use crate::{
    account::Load,
    backend::Backend,
    icons::icon,
    media::Media,
    session::Session,
    theme,
};

/// Newer emoji than this show as empty boxes in the system font, so they
/// are left out rather than offered as something that can't be seen.
const NEWEST_DRAWABLE: emojis::UnicodeVersion = emojis::UnicodeVersion::new(13, 0);

/// Most results drawn for a search, so a one letter query stays quick.
const MAX_RESULTS: usize = 160;
const CELL: f32 = 34.;

/// The user chose an emoji. Unicode ones carry their character as the name.
pub struct Picked(pub Emoji);

/// A GIF was chosen. Posting it is the owner's business.
pub struct PickedGif(pub Gif);

impl EventEmitter<PickedGif> for EmojiPanel {}

/// A sticker was chosen, to be sent on its own.
pub struct PickedSticker(pub StickerItem);

impl EventEmitter<PickedSticker> for EmojiPanel {}

/// Which picker is showing.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Emoji,
    Gifs,
    Stickers,
}

/// The panel wants to go away.
pub struct ClosePicker;

impl EventEmitter<Picked> for EmojiPanel {}
impl EventEmitter<ClosePicker> for EmojiPanel {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tab {
    /// The emoji of the open server.
    Server,
    Unicode(Group),
}

impl Tab {
    fn glyph(self) -> &'static str {
        match self {
            Tab::Server => "server",
            Tab::Unicode(Group::SmileysAndEmotion) => "face-slightly-smiling",
            Tab::Unicode(Group::PeopleAndBody) => "users",
            Tab::Unicode(Group::AnimalsAndNature) => "paw-print",
            Tab::Unicode(Group::FoodAndDrink) => "utensils",
            Tab::Unicode(Group::TravelAndPlaces) => "plane",
            Tab::Unicode(Group::Activities) => "trophy",
            Tab::Unicode(Group::Objects) => "lightbulb",
            Tab::Unicode(Group::Symbols) => "shapes",
            Tab::Unicode(Group::Flags) => "flag",
        }
    }

    fn title(self) -> &'static str {
        match self {
            Tab::Server => "Server emoji",
            Tab::Unicode(Group::SmileysAndEmotion) => "Smileys & emotion",
            Tab::Unicode(Group::PeopleAndBody) => "People & body",
            Tab::Unicode(Group::AnimalsAndNature) => "Animals & nature",
            Tab::Unicode(Group::FoodAndDrink) => "Food & drink",
            Tab::Unicode(Group::TravelAndPlaces) => "Travel & places",
            Tab::Unicode(Group::Activities) => "Activities",
            Tab::Unicode(Group::Objects) => "Objects",
            Tab::Unicode(Group::Symbols) => "Symbols",
            Tab::Unicode(Group::Flags) => "Flags",
        }
    }
}

const GROUPS: [Group; 9] = [
    Group::SmileysAndEmotion,
    Group::PeopleAndBody,
    Group::AnimalsAndNature,
    Group::FoodAndDrink,
    Group::TravelAndPlaces,
    Group::Activities,
    Group::Objects,
    Group::Symbols,
    Group::Flags,
];

/// One cell of the grid.
enum Item {
    Unicode(&'static emojis::Emoji),
    Custom(Emoji),
}

pub struct EmojiPanel {
    session: Entity<Session>,
    media: Entity<Media>,
    backend: Backend,
    mode: Mode,
    search: Entity<InputState>,
    tab: Tab,
    /// The server whose emoji the first tab lists.
    guild: Option<Snowflake>,
    _subscriptions: Vec<Subscription>,
}

impl EmojiPanel {
    pub fn new(
        session: Entity<Session>,
        media: Entity<Media>,
        backend: Backend,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search emoji"));
        let subscriptions = vec![
            cx.observe(&search, |_, _, cx| cx.notify()),
            cx.observe(&media, |_, _, cx| cx.notify()),
            cx.observe(&session, |_, _, cx| cx.notify()),
            // GIF searches go to the server, so they wait for Enter.
            cx.subscribe_in(&search, window, |this, _, event: &InputEvent, _, cx| {
                if this.mode == Mode::Gifs && matches!(event, InputEvent::PressEnter { .. }) {
                    this.search_gifs(cx);
                }
            }),
        ];
        Self {
            session,
            media,
            backend,
            mode: Mode::Emoji,
            search,
            tab: Tab::Unicode(Group::SmileysAndEmotion),
            guild: None,
            _subscriptions: subscriptions,
        }
    }

    fn set_mode(&mut self, mode: Mode, window: &mut Window, cx: &mut Context<Self>) {
        self.mode = mode;
        self.search.update(cx, |input, cx| {
            let hint = match mode {
                Mode::Emoji => "Search emoji",
                Mode::Gifs => "Search GIFs",
                Mode::Stickers => "Search stickers",
            };
            input.set_placeholder(hint, window, cx);
            input.set_value("", window, cx);
            input.focus(window, cx);
        });
        if mode == Mode::Gifs && matches!(self.session.read(cx).gifs, Load::Idle | Load::Failed(_)) {
            let backend = self.backend.clone();
            self.session.update(cx, |s, cx| s.request_gifs(&backend, None, cx));
        }
        cx.notify();
    }

    fn search_gifs(&mut self, cx: &mut Context<Self>) {
        let text = self.search.read(cx).value().trim().to_owned();
        let query = (!text.is_empty()).then_some(text);
        let backend = self.backend.clone();
        self.session.update(cx, |s, cx| s.request_gifs(&backend, query, cx));
    }

    /// The GIFs, Stickers and Emoji switch at the top, in Discord's order.
    fn render_modes(&mut self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let mut row = div().flex().gap_1();
        for (mode, label) in [(Mode::Gifs, "GIFs"), (Mode::Stickers, "Stickers"), (Mode::Emoji, "Emoji")] {
            let selected = self.mode == mode;
            row = row.child(
                div()
                    .id(label)
                    .h(px(26.))
                    .px_3()
                    .flex()
                    .items_center()
                    .rounded(px(theme::RADIUS))
                    .cursor_pointer()
                    .text_size(px(13.))
                    .text_color(rgb(if selected { theme::TEXT_BRIGHT } else { theme::TEXT_MUTED }))
                    .bg(rgb(if selected { theme::BG_ACTIVE } else { theme::BG_PANEL }))
                    .hover(|s| s.bg(rgb(theme::BG_HOVER)))
                    .on_click(cx.listener(move |this, _, window, cx| this.set_mode(mode, window, cx)))
                    .child(label),
            );
        }
        row
    }

    /// The grid of GIF thumbnails, two to a row.
    fn render_gifs(&mut self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let note = |text: String| {
            div().p_2().text_color(rgb(theme::TEXT_MUTED)).child(text).into_any_element()
        };
        match self.session.read(cx).gifs.clone() {
            Load::Ready(list) if list.is_empty() => note("No GIFs found.".into()),
            Load::Ready(list) => {
                let mut grid = div().flex().flex_wrap().gap(px(4.));
                for (i, gif) in list.into_iter().enumerate() {
                    let image = self.media.update(cx, |m, _| m.get(&gif.preview));
                    grid = grid.child(
                        div()
                            .id(("gif", i as u64))
                            .w(px(166.))
                            .h(px(110.))
                            .rounded(px(theme::RADIUS))
                            .overflow_hidden()
                            .bg(rgb(theme::BG_SIDEBAR))
                            .cursor_pointer()
                            .hover(|s| s.opacity(0.85))
                            .on_click(cx.listener(move |_, _, _, cx| cx.emit(PickedGif(gif.clone()))))
                            .children(image.map(|image| {
                                img(gpui_kit::ImageSource::Image(image))
                                    .size_full()
                                    .object_fit(gpui_kit::ObjectFit::Cover)
                            })),
                    );
                }
                grid.into_any_element()
            }
            Load::Failed(error) => note(format!("Could not load GIFs. {error}")),
            _ => note("Loading GIFs...".into()),
        }
    }

    /// The open server's stickers matching the search, three to a row. Ones
    /// the server can no longer afford are shown faded and can't be picked.
    fn render_stickers(&mut self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let note = |text: &str| {
            div().p_2().text_color(rgb(theme::TEXT_MUTED)).child(text.to_owned()).into_any_element()
        };
        let Some(guild) = self.guild else {
            return note("Stickers from a server can be sent inside that server.");
        };
        let query = self.search.read(cx).value().trim().to_lowercase();
        let stickers: Vec<StickerItem> = self
            .session
            .read(cx)
            .store
            .guild(guild)
            .map(|g| g.stickers.clone())
            .unwrap_or_default()
            .into_iter()
            .filter(|s| s.name.to_lowercase().contains(&query))
            .collect();
        if stickers.is_empty() {
            return note(if query.is_empty() { "This server has no stickers." } else { "No stickers found." });
        }

        let mut grid = div().flex().flex_wrap().gap(px(4.));
        for (i, sticker) in stickers.into_iter().enumerate() {
            let cell = div()
                .id(("sticker", i as u64))
                .size(px(108.))
                .p_1()
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(theme::RADIUS))
                .text_size(px(12.))
                .text_color(rgb(theme::TEXT_MUTED));
            let cell = if sticker.available {
                cell.cursor_pointer().hover(|s| s.bg(rgb(theme::BG_HOVER)))
            } else {
                cell.opacity(0.4)
            };
            let image = (!sticker.is_lottie()).then(|| {
                let url = cdn::sticker(sticker.id, sticker.extension(), 160);
                self.media.update(cx, |m, _| m.get(&url))
            });
            let cell = match image {
                Some(image) => cell.children(image.map(|image| {
                    img(gpui_kit::ImageSource::Image(image))
                        .size_full()
                        .object_fit(gpui_kit::ObjectFit::Contain)
                })),
                // A vector animation we cannot play, so it goes by name.
                None => cell.child(sticker.name.clone()),
            };
            let cell = if sticker.available {
                cell.on_click(cx.listener(move |_, _, _, cx| cx.emit(PickedSticker(sticker.clone()))))
            } else {
                cell
            };
            grid = grid.child(cell);
        }
        grid.into_any_element()
    }

    /// Opens straight onto the sticker grid, for the composer's sticker button.
    pub fn show_stickers(&mut self, guild: Option<Snowflake>, window: &mut Window, cx: &mut Context<Self>) {
        self.show(guild, window, cx);
        self.set_mode(Mode::Stickers, window, cx);
    }

    /// Opens on the right tab for `guild` and focuses the search box.
    pub fn show(&mut self, guild: Option<Snowflake>, window: &mut Window, cx: &mut Context<Self>) {
        self.mode = Mode::Emoji;
        self.guild = guild;
        let has_custom = !self.custom(cx).is_empty();
        self.tab = if has_custom { Tab::Server } else { Tab::Unicode(Group::SmileysAndEmotion) };
        self.search.update(cx, |input, cx| {
            input.set_placeholder("Search emoji", window, cx);
            input.set_value("", window, cx);
            input.focus(window, cx);
        });
        cx.notify();
    }

    /// Opens straight onto the GIF grid, for the composer's GIF button.
    pub fn show_gifs(&mut self, guild: Option<Snowflake>, window: &mut Window, cx: &mut Context<Self>) {
        self.show(guild, window, cx);
        self.set_mode(Mode::Gifs, window, cx);
    }

    fn custom(&self, cx: &gpui_kit::App) -> Vec<Emoji> {
        let Some(guild) = self.guild else { return Vec::new() };
        self.session
            .read(cx)
            .store
            .guild(guild)
            .map(|g| g.emojis.iter().filter(|e| e.id.is_some()).cloned().collect())
            .unwrap_or_default()
    }

    fn items(&self, cx: &gpui_kit::App) -> (SharedString, Vec<Item>) {
        let query = self.search.read(cx).value().trim().to_lowercase();
        if !query.is_empty() {
            let mut found: Vec<Item> = self
                .custom(cx)
                .into_iter()
                .filter(|e| e.name.as_deref().is_some_and(|n| n.to_lowercase().contains(&query)))
                .map(Item::Custom)
                .collect();
            found.extend(
                emojis::iter()
                    .filter(|e| e.unicode_version() <= NEWEST_DRAWABLE)
                    .filter(|e| {
                        e.name().contains(&query) || e.shortcodes().any(|s| s.contains(&query))
                    })
                    .map(Item::Unicode),
            );
            found.truncate(MAX_RESULTS);
            return ("Search results".into(), found);
        }
        let items = match self.tab {
            Tab::Server => self.custom(cx).into_iter().map(Item::Custom).collect(),
            Tab::Unicode(group) => group
                .emojis()
                .filter(|e| e.unicode_version() <= NEWEST_DRAWABLE)
                .map(Item::Unicode)
                .collect(),
        };
        (self.tab.title().into(), items)
    }

    fn pick(&mut self, emoji: Emoji, cx: &mut Context<Self>) {
        cx.emit(Picked(emoji));
    }
}

impl Render for EmojiPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (title, items) = self.items(cx);

        let mut tabs = div().flex().items_center().gap(px(2.));
        let mut order = Vec::new();
        if !self.custom(cx).is_empty() {
            order.push(Tab::Server);
        }
        order.extend(GROUPS.map(Tab::Unicode));
        for (index, tab) in order.into_iter().enumerate() {
            let selected = self.tab == tab;
            tabs = tabs.child(
                div()
                    .id(("emoji-tab", index as u64))
                    .size(px(28.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(theme::RADIUS))
                    .cursor_pointer()
                    .bg(rgb(if selected { theme::BG_ACTIVE } else { theme::BG_PANEL }))
                    .hover(|s| s.bg(rgb(theme::BG_HOVER)))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.tab = tab;
                        this.search.update(cx, |i, cx| i.set_value("", window, cx));
                        cx.notify();
                    }))
                    .child(icon(tab.glyph(), 16., if selected { theme::TEXT_BRIGHT } else { theme::TEXT_MUTED })),
            );
        }

        let mut grid = div().flex().flex_wrap().gap(px(2.));
        for (i, item) in items.into_iter().enumerate() {
            let cell = div()
                .id(("emoji-cell", i as u64))
                .size(px(CELL))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(theme::RADIUS))
                .cursor_pointer()
                .hover(|s| s.bg(rgb(theme::BG_HOVER)));
            grid = grid.child(match item {
                Item::Unicode(e) => {
                    let glyph = e.as_str().to_owned();
                    let emoji = Emoji { name: Some(glyph.clone()), ..Emoji::default() };
                    cell.text_size(px(22.))
                        .on_click(cx.listener(move |this, _, _, cx| this.pick(emoji.clone(), cx)))
                        .child(glyph)
                        .into_any_element()
                }
                Item::Custom(e) => {
                    let url = cdn::emoji(e.id.unwrap_or(Snowflake(0)), 64);
                    let image = self.media.update(cx, |m, _| m.get(&url));
                    cell.on_click(cx.listener(move |this, _, _, cx| this.pick(e.clone(), cx)))
                        .children(image.map(|image| {
                            img(gpui_kit::ImageSource::Image(image)).size(px(26.))
                        }))
                        .into_any_element()
                }
            });
        }

        let emoji_view = self.mode == Mode::Emoji;
        let body = match self.mode {
            Mode::Emoji => grid.into_any_element(),
            Mode::Gifs => self.render_gifs(cx),
            Mode::Stickers => self.render_stickers(cx),
        };
        let modes = self.render_modes(cx);

        div()
            .id("emoji-panel")
            .occlude()
            .w(px(360.))
            .h(px(440.))
            .flex()
            .flex_col()
            .gap_2()
            .p_2()
            .rounded(px(theme::RADIUS))
            .border_1()
            .border_color(rgb(theme::BORDER))
            .bg(rgb(theme::BG_PANEL))
            .child(modes)
            .child(Input::new(&self.search))
            .children(emoji_view.then_some(tabs))
            .children(emoji_view.then(|| {
                div()
                    .text_size(px(11.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(theme::TEXT_MUTED))
                    .child(title.to_uppercase())
            }))
            .child(div().id("emoji-grid").flex_1().min_h_0().overflow_y_scroll().child(body))
    }
}
