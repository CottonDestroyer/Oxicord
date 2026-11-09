//! The Games I Like widget editor the Edit Profile board shows inline, as the
//! live client draws it (captured 2026-11-09, 1920 x 1080, dark theme).

use gpui_kit::{
    AnyElement, AppContext as _, ClickEvent, Context, Entity, FontWeight, ImageSource,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, StatefulInteractiveElement as _,
    Styled as _, StyledImage as _, Subscription, Window, div, img, prelude::FluentBuilder as _, px,
    rgb, rgba,
    component::input::{Input, InputEvent, InputState},
};

use crate::{icons::icon, media::Media, theme};

/// The faint wash the add button and empty covers use.
const WASH: u32 = 0x97979f1f;

/// The border colour the cards and menus share.
const LINE: u32 = 0x97979f33;

/// The most games the widget holds.
pub const MAX_GAMES: usize = 20;

struct Game {
    name: &'static str,
    /// The game's Discord application id, as the save request names it.
    id: &'static str,
    cover: Option<&'static str>,
}

/// The Games I Like browse list and suggestions, read from the live client on
/// 2026-11-09.
const GAMES: &[Game] = &[
    Game {
        name: "Dark and Darker",
        id: "1124351715618074634",
        cover: Some("https://images-ext-1.discordapp.net/external/M6xDnAX0Nc5T0PCxewyyLoutsnFKcD7C6_sDEJ0k2JY/https/images.igdb.com/igdb/image/upload/t_cover_big/co5byi.jpg?keep_aspect_ratio=true&format=webp"),
    },
    Game {
        name: "Chained Together",
        id: "1253061774555611238",
        cover: Some("https://images-ext-1.discordapp.net/external/_RZwEN5eCsdp2GQ8UFlUtqwwoo3ohZtRAEm5mz_Ro_0/https/images.igdb.com/igdb/image/upload/t_cover_big/coaazz.jpg?keep_aspect_ratio=true&format=webp"),
    },
    Game {
        name: "Sid Meier's Civilization VI",
        id: "363413834301571072",
        cover: Some("https://images-ext-1.discordapp.net/external/VX7VqiwaW5R-093tG35LhGXBFJ5gz09tv6Kf0erLUa0/https/images.igdb.com/igdb/image/upload/t_cover_big/coaaqr.jpg?keep_aspect_ratio=true&format=webp"),
    },
    Game {
        name: "MapleStory",
        id: "359510095811444736",
        cover: Some("https://images-ext-1.discordapp.net/external/Nd3iAupGG6BD0gO-V3R0kDvxDHzMXWMqHRe2wCVfUhI/https/images.igdb.com/igdb/image/upload/t_cover_big/cod8pl.jpg?keep_aspect_ratio=true&format=webp"),
    },
    Game {
        name: "Left 4 Dead 2",
        id: "356954277803065354",
        cover: Some("https://images-ext-1.discordapp.net/external/qrUVYot4n-cV-mLInND922JAkv1gXn_mnwlwEyj64jQ/https/images.igdb.com/igdb/image/upload/t_cover_big/co1y2f.jpg?keep_aspect_ratio=true&format=webp"),
    },
    Game {
        name: "Call of Duty®: MWIII",
        id: "1402418487745581248",
        cover: Some("https://cdn.discordapp.com/app-icons/1402418487745581248/7dd9116c1287c1469cd14329cbc9c46f.webp?size=128&keep_aspect_ratio=true"),
    },
    Game {
        name: "Battlefield 2042",
        id: "1162076274622222346",
        cover: Some("https://images-ext-1.discordapp.net/external/EV2QFnhfhisEghXKOwC56ABebcbdFlkxOfDf1lraKoo/https/images.igdb.com/igdb/image/upload/t_cover_big/co7dfn.jpg?keep_aspect_ratio=true&format=webp"),
    },
    Game { name: "ROBLOX", id: "363445589247131668", cover: None },
    Game { name: "VALORANT", id: "700136079562375258", cover: None },
    Game { name: "League of Legends", id: "1402418696126992445", cover: None },
    Game { name: "Minecraft", id: "1402418491272986635", cover: None },
    Game { name: "Counter-Strike 2", id: "1158877933042143272", cover: None },
    Game { name: "Fortnite", id: "1402418703554842694", cover: None },
    Game { name: "Grand Theft Auto V Legacy", id: "1402418714716143646", cover: None },
    Game {
        name: "Overwatch",
        id: "356875221078245376",
        cover: Some("https://cdn.discordapp.com/app-icons/356875221078245376/843a3b07639f068fdacf40b9c3808c46.webp?size=128&keep_aspect_ratio=true"),
    },
    Game { name: "Dota 2", id: "356875988589740042", cover: None },
    Game { name: "Rocket League", id: "356877880938070016", cover: None },
    Game { name: "PUBG: BATTLEGROUNDS", id: "1402418717781921935", cover: None },
    Game { name: "Apex Legends", id: "542075586886107149", cover: None },
    Game { name: "Genshin Impact", id: "762434991303950386", cover: None },
    Game { name: "ELDEN RING", id: "1402418436809953330", cover: None },
];

/// The suggested covers in the live empty editor.
const SUGGESTED: &[usize] = &[0, 1, 2, 3, 4, 5];

pub struct GamesILikeEditor {
    media: Entity<Media>,
    search: Entity<InputState>,
    browse_open: bool,
    /// The suggestions block is dismissed for this open.
    suggestions_open: bool,
    /// Chosen game indices into GAMES, in the order added.
    chosen: Vec<usize>,
    _subscriptions: Vec<Subscription>,
}

impl GamesILikeEditor {
    pub fn new(media: Entity<Media>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search"));
        let subscriptions =
            vec![cx.subscribe(&search, |_, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            })];
        Self {
            media,
            search,
            browse_open: false,
            suggestions_open: true,
            chosen: Vec::new(),
            _subscriptions: subscriptions,
        }
    }

    /// A fresh editor, as the live client shows it when the card is picked.
    pub fn opened(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.browse_open = false;
        self.suggestions_open = true;
        self.chosen.clear();
        self.search
            .update(cx, |state, cx| state.set_value("", window, cx));
        cx.notify();
    }

    fn add(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.chosen.len() >= MAX_GAMES || self.chosen.contains(&index) {
            return;
        }
        self.chosen.push(index);
        self.browse_open = false;
        cx.notify();
    }

    fn remove(&mut self, index: usize, cx: &mut Context<Self>) {
        self.chosen.retain(|&i| i != index);
        cx.notify();
    }

    /// The chosen games, as the board card draws them.
    pub fn chosen(&self) -> Vec<(String, Option<String>)> {
        self.chosen
            .iter()
            .map(|&index| (GAMES[index].name.to_owned(), GAMES[index].cover.map(str::to_owned)))
            .collect()
    }

    /// The `PUT /users/@me/widgets` entry for the chosen games. Built here so
    /// the shape is exercised, but never sent from the real account.
    pub fn submission(&self) -> Option<oxicord_model::WidgetSubmission> {
        if self.chosen.is_empty() {
            return None;
        }
        let games: Vec<_> = self
            .chosen
            .iter()
            .filter_map(|&index| {
                GAMES[index]
                    .id
                    .parse()
                    .ok()
                    .map(|game_id| oxicord_model::GameEntrySubmission {
                        game_id,
                        comment: String::new(),
                        tags: Vec::new(),
                    })
            })
            .collect();
        if games.is_empty() {
            return None;
        }
        Some(oxicord_model::WidgetSubmission {
            id: None,
            data: oxicord_model::WidgetDataSubmission::PlayedGames { games },
        })
    }

    fn tile(&self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let group = format!("like-tile-{index}");
        let mut cover = div()
            .w(px(87.25))
            .h(px(116.33))
            .flex_none()
            .rounded(px(8.))
            .overflow_hidden()
            .bg(rgba(WASH));
        if let Some(url) = GAMES[index].cover
            && let Some(image) = self.media.update(cx, |m, _| m.get(url))
        {
            cover = cover.child(
                img(ImageSource::Image(image))
                    .size_full()
                    .object_fit(gpui_kit::ObjectFit::Cover),
            );
        }
        div()
            .id(gpui_kit::ElementId::Name(format!("like-game-{index}").into()))
            .relative()
            .w(px(87.25))
            .h(px(116.33))
            .flex_none()
            .group(gpui_kit::SharedString::from(group.clone()))
            .child(cover)
            .child(
                div()
                    .id(gpui_kit::ElementId::Name(format!("remove-like-{index}").into()))
                    .absolute()
                    .top(px(4.))
                    .right(px(4.))
                    .size(px(32.))
                    .rounded(px(8.))
                    .border_1()
                    .border_color(rgba(0x97979f29))
                    .bg(rgba(0x0000008f))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .invisible()
                    .group_hover(gpui_kit::SharedString::from(group), |d| d.visible())
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.remove(index, cx)
                    }))
                    .child(icon("trash", 16., 0xffff_ffff)),
            )
            .into_any_element()
    }

    fn popover(&self, cx: &mut Context<Self>) -> AnyElement {
        let query = self.search.read(cx).value().trim().to_lowercase();
        let mut rows = Vec::new();
        for (index, game) in GAMES.iter().enumerate() {
            if !query.is_empty() && !game.name.to_lowercase().contains(&query) {
                continue;
            }
            let picked = self.chosen.contains(&index);
            rows.push(
                div()
                    .id(gpui_kit::ElementId::Name(format!("like-option-{index}").into()))
                    .h(px(40.))
                    .w_full()
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_between()
                    .rounded(px(4.))
                    .px(px(8.))
                    .py(px(3.))
                    .cursor_pointer()
                    .text_size(px(16.))
                    .line_height(px(20.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(0xe4e5e7))
                    .hover(|d| d.bg(rgba(WASH)))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.add(index, cx)
                    }))
                    .child(game.name)
                    .when(picked, |d| d.child(icon("check", 24., theme::TEXT_BRIGHT)))
                    .into_any_element(),
            );
        }

        div()
            .absolute()
            .left(px(199.))
            .top(px(53.))
            .w(px(398.))
            .flex()
            .flex_col()
            .rounded(px(8.))
            .border_1()
            .border_color(rgba(LINE))
            .bg(rgb(theme::BG_MENU))
            .p(px(8.))
            .child(
                div()
                    .h(px(40.))
                    .w_full()
                    .flex_none()
                    .flex()
                    .items_center()
                    .px(px(12.))
                    .child(Input::new(&self.search).appearance(false).bordered(false)),
            )
            .child(
                div()
                    .id("like-browse-games")
                    .h(px(288.))
                    .w_full()
                    .flex_none()
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .p(px(4.))
                    .children(rows),
            )
            .into_any_element()
    }

    fn suggestions(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut covers = div().flex().gap(px(12.));
        for &index in SUGGESTED {
            if self.chosen.contains(&index) {
                continue;
            }
            let mut tile = div()
                .id(GAMES[index].name)
                .w(px(56.16))
                .h(px(74.89))
                .flex_none()
                .rounded(px(8.))
                .overflow_hidden()
                .bg(rgba(WASH))
                .cursor_pointer()
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.add(index, cx)));
            if let Some(url) = GAMES[index].cover
                && let Some(image) = self.media.update(cx, |m, _| m.get(url))
            {
                tile = tile.child(
                    img(ImageSource::Image(image))
                        .size_full()
                        .object_fit(gpui_kit::ObjectFit::Cover),
                );
            }
            covers = covers.child(tile);
        }

        div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .mt(px(4.))
            .pt(px(16.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(12.))
                            .line_height(px(16.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgb(theme::TEXT_MUTED))
                            .child("Suggested for you"),
                    )
                    .child(
                        div()
                            .id("dismiss-like-suggestions")
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.suggestions_open = false;
                                cx.notify();
                            }))
                            .child(icon("x", 18., theme::TEXT_MUTED)),
                    ),
            )
            .child(covers)
            .into_any_element()
    }
}

impl Render for GamesILikeEditor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let chosen = !self.chosen.is_empty();

        let title = div()
            .flex()
            .flex_col()
            .justify_center()
            .child(
                div()
                    .text_size(px(14.))
                    .line_height(px(18.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(theme::TEXT_BRIGHT))
                    .child("Games I Like"),
            )
            .when(chosen, |d| {
                d.child(
                    div()
                        .text_size(px(12.))
                        .line_height(px(16.))
                        .text_color(rgb(theme::TEXT_MUTED))
                        .child("Add up to 20 games"),
                )
            });

        let add = div()
            .id("add-like-game")
            .size(px(32.))
            .flex_none()
            .rounded(px(8.))
            .border_1()
            .border_color(rgba(0x97979f29))
            .bg(rgba(WASH))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.browse_open = !this.browse_open;
                cx.notify();
            }))
            .child(icon("plus", 16., theme::TEXT_BRIGHT));

        let mut card = div()
            .relative()
            .w(px(431.))
            .flex_none()
            .flex()
            .flex_col()
            .gap(px(8.))
            .rounded(px(16.))
            .bg(rgb(theme::BG_CARD))
            .border_1()
            .border_color(rgba(0x97979f29))
            .p(px(16.))
            .pt(px(12.))
            .child(
                div()
                    .h(px(if chosen { 34. } else { 32. }))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(title)
                    .child(add),
            );

        if chosen {
            let mut grid = div().w_full().flex().flex_wrap().gap(px(16.));
            for &index in &self.chosen {
                grid = grid.child(self.tile(index, cx));
            }
            card = card.child(grid);
        } else {
            card = card.child(
                div()
                    .py(px(16.))
                    .w_full()
                    .text_size(px(12.))
                    .line_height(px(16.))
                    .text_color(rgb(theme::TEXT_MUTED))
                    .child("Add up to 20 games. This Widget won't show up on your profile until you add at least 1 game."),
            );
        }

        if self.suggestions_open {
            card = card.child(self.suggestions(cx));
        }
        if self.browse_open {
            card = card.child(self.popover(cx));
        }
        card
    }
}
