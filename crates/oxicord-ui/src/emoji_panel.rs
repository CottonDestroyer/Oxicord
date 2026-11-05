//! The emoji picker: a search box, category tabs and a grid. It is shared by
//! the composer and by reactions, the owner decides what a pick does.

use emojis::Group;
use gpui_kit::{
    AppContext as _, Context, Entity, EventEmitter, FontWeight, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, SharedString, StatefulInteractiveElement as _,
    Styled as _, Subscription, Window, div, img, px, rgb,
    component::input::{Input, InputState},
};
use oxicord_media::cdn;
use oxicord_model::{Emoji, Snowflake};

use crate::{icons::icon, media::Media, session::Session, theme};

/// Newer emoji than this show as empty boxes in the system font, so they
/// are left out rather than offered as something that can't be seen.
const NEWEST_DRAWABLE: emojis::UnicodeVersion = emojis::UnicodeVersion::new(13, 0);

/// Most results drawn for a search, so a one letter query stays quick.
const MAX_RESULTS: usize = 160;
const CELL: f32 = 34.;

/// The user chose an emoji. Unicode ones carry their character as the name.
pub struct Picked(pub Emoji);

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
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search emoji"));
        let subscriptions = vec![
            cx.observe(&search, |_, _, cx| cx.notify()),
            cx.observe(&media, |_, _, cx| cx.notify()),
        ];
        Self {
            session,
            media,
            search,
            tab: Tab::Unicode(Group::SmileysAndEmotion),
            guild: None,
            _subscriptions: subscriptions,
        }
    }

    /// Opens on the right tab for `guild` and focuses the search box.
    pub fn show(&mut self, guild: Option<Snowflake>, window: &mut Window, cx: &mut Context<Self>) {
        self.guild = guild;
        let has_custom = !self.custom(cx).is_empty();
        self.tab = if has_custom { Tab::Server } else { Tab::Unicode(Group::SmileysAndEmotion) };
        self.search.update(cx, |input, cx| {
            input.set_value("", window, cx);
            input.focus(window, cx);
        });
        cx.notify();
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
                    .rounded(px(6.))
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
                .rounded(px(6.))
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

        div()
            .id("emoji-panel")
            .occlude()
            .w(px(360.))
            .h(px(400.))
            .flex()
            .flex_col()
            .gap_2()
            .p_2()
            .rounded(px(10.))
            .border_1()
            .border_color(rgb(theme::BORDER))
            .bg(rgb(theme::BG_PANEL))
            .child(Input::new(&self.search))
            .child(tabs)
            .child(
                div()
                    .text_size(px(11.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(theme::TEXT_MUTED))
                    .child(title.to_uppercase()),
            )
            .child(div().id("emoji-grid").flex_1().min_h_0().overflow_y_scroll().child(grid))
    }
}
