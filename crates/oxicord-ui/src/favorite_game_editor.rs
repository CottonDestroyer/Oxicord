//! The Favorite Game widget editor the Edit Profile board shows inline, as
//! the live client draws it (captured 2026-11-09, 1920 x 1080, dark theme).

use gpui_kit::{
    AnyElement, AppContext as _, ClickEvent, Context, Entity, FontWeight, ImageSource,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, StatefulInteractiveElement as _,
    Styled as _, StyledImage as _, Subscription, Window, div, img, prelude::FluentBuilder as _, px,
    rgb, rgba,
    component::input::{Input, InputEvent, InputState},
};

use crate::{icons::icon, media::Media, theme};

/// The faint wash the add tile and the empty covers use.
const WASH: u32 = 0x97979f1f;

struct Game {
    name: &'static str,
    cover: Option<&'static str>,
}

/// The games the live browse list and suggestions contained on 2026-11-09.
const GAMES: &[Game] = &[
    Game {
        name: "Lethal Company",
        cover: Some("https://images-ext-1.discordapp.net/external/kxdIMrNToiGKbJccIo93PX6wHpdMHbkms0kwiOWl6Bs/https/images.igdb.com/igdb/image/upload/t_cover_big/co5ive.jpg?keep_aspect_ratio=true&format=webp"),
    },
    Game {
        name: "Overwatch",
        cover: Some("https://cdn.discordapp.com/app-icons/356875221078245376/843a3b07639f068fdacf40b9c3808c46.webp?size=128&keep_aspect_ratio=true"),
    },
    Game {
        name: "Stardew Valley",
        cover: Some("https://cdn.discordapp.com/app-icons/359509387670192128/2727f218e06d9c3c4204fed086181a53.webp?size=128&keep_aspect_ratio=true"),
    },
    Game {
        name: "Destiny 2",
        cover: Some("https://images-ext-1.discordapp.net/external/KRe_8xMkBNXe0t5RS8_aQc5WqI4Rgl2Sypdf_VLfJVk/https/images.igdb.com/igdb/image/upload/t_cover_big/coc89g.jpg?keep_aspect_ratio=true&format=webp"),
    },
    Game {
        name: "Among Us",
        cover: Some("https://images-ext-1.discordapp.net/external/vwjJ-pK7-GEXq_yHs8oOoJ5Zplu_7VP8dZIZEnW6jLQ/https/images.igdb.com/igdb/image/upload/t_cover_big/co6kqt.jpg?keep_aspect_ratio=true&format=webp"),
    },
    Game {
        name: "Escape from Tarkov: Arena",
        cover: Some("https://images-ext-1.discordapp.net/external/izydqalQpGYCOoUOURoSyS7t-RsETqZ7AWuLsUYv48M/https/images.igdb.com/igdb/image/upload/t_cover_big/cocxnk.jpg?keep_aspect_ratio=true&format=webp"),
    },
    Game { name: "ROBLOX", cover: None },
    Game { name: "VALORANT", cover: None },
    Game { name: "League of Legends", cover: None },
    Game { name: "Minecraft", cover: None },
    Game { name: "Counter-Strike 2", cover: None },
    Game { name: "Fortnite", cover: None },
    Game { name: "Grand Theft Auto V Legacy", cover: None },
    Game { name: "Dota 2", cover: None },
    Game { name: "Rocket League", cover: None },
    Game { name: "Call of Duty®: MWIII", cover: None },
    Game { name: "PUBG: BATTLEGROUNDS", cover: None },
    Game { name: "Apex Legends", cover: None },
    Game { name: "Genshin Impact", cover: None },
    Game { name: "ELDEN RING", cover: None },
];

/// The suggested covers, which are the first six games.
const SUGGESTED: usize = 6;

pub struct FavoriteGameEditor {
    media: Entity<Media>,
    search: Entity<InputState>,
    browse_open: bool,
    selected: Option<usize>,
    _subscriptions: Vec<Subscription>,
}

impl FavoriteGameEditor {
    pub fn new(media: Entity<Media>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search"));
        let subscriptions = vec![cx.subscribe(&search, |_, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        })];
        Self { media, search, browse_open: false, selected: None, _subscriptions: subscriptions }
    }

    /// A fresh editor, as the live client shows it when the card is picked.
    pub fn opened(&mut self, cx: &mut Context<Self>) {
        self.browse_open = false;
        self.selected = None;
        cx.notify();
    }

    fn pick(&mut self, index: usize, cx: &mut Context<Self>) {
        self.selected = Some(index);
        self.browse_open = false;
        cx.notify();
    }

    fn game_tile(&self, cx: &mut Context<Self>) -> AnyElement {
        match self.selected.and_then(|index| GAMES[index].cover) {
            Some(url) => match self.media.update(cx, |m, _| m.get(url)) {
                Some(image) => img(ImageSource::Image(image))
                    .w(px(87.25))
                    .h(px(116.33))
                    .flex_none()
                    .object_fit(gpui_kit::ObjectFit::Cover)
                    .into_any_element(),
                None => div().w(px(87.25)).h(px(116.33)).flex_none().bg(rgba(WASH)).into_any_element(),
            },
            None => div()
                .id("add-game")
                .w(px(87.25))
                .h(px(116.33))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .bg(rgba(WASH))
                .cursor_pointer()
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.browse_open = !this.browse_open;
                    cx.notify();
                }))
                .child(icon("plus", 24., theme::TEXT_MUTED))
                .into_any_element(),
        }
    }

    fn popover(&self, cx: &mut Context<Self>) -> AnyElement {
        let query = self.search.read(cx).value().trim().to_lowercase();
        let mut rows = Vec::new();
        for (index, game) in GAMES.iter().enumerate() {
            if !query.is_empty() && !game.name.to_lowercase().contains(&query) {
                continue;
            }
            let picked = self.selected == Some(index);
            rows.push(
                div()
                    .id(gpui_kit::ElementId::Name(format!("game-{index}").into()))
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
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.pick(index, cx)))
                    .child(game.name)
                    .when(picked, |d| d.child(icon("check", 24., theme::TEXT_BRIGHT)))
                    .into_any_element(),
            );
        }

        div()
            .absolute()
            .left(px(111.25))
            .top(px(47.))
            .w(px(398.))
            .flex()
            .flex_col()
            .rounded(px(8.))
            .border_1()
            .border_color(rgba(0x97979f33))
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
                    .id("browse-games")
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
}

impl Render for FavoriteGameEditor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut covers = div().flex().gap(px(12.));
        for (index, game) in GAMES.iter().enumerate().take(SUGGESTED) {
            let mut tile = div()
                .id(game.name)
                .w(px(56.2))
                .h(px(74.9))
                .flex_none()
                .rounded(px(8.))
                .overflow_hidden()
                .bg(rgba(WASH))
                .cursor_pointer()
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.pick(index, cx)));
            if let Some(url) = game.cover
                && let Some(image) = self.media.update(cx, |m, _| m.get(url))
            {
                tile = tile.child(
                    img(ImageSource::Image(image)).size_full().object_fit(gpui_kit::ObjectFit::Cover),
                );
            }
            covers = covers.child(tile);
        }

        let details = match self.selected {
            Some(index) => div()
                .text_size(px(14.))
                .line_height(px(20.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(rgb(theme::TEXT_BRIGHT))
                .child(GAMES[index].name),
            None => div()
                .text_size(px(12.))
                .line_height(px(16.))
                .text_color(rgb(theme::TEXT_MUTED))
                .child("Add one game. This Widget won't show up on your profile until you add a game."),
        };

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
                    .text_size(px(14.))
                    .line_height(px(18.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(theme::TEXT_BRIGHT))
                    .child("Favorite Game"),
            )
            .child(
                div()
                    .h(px(132.3))
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap(px(16.))
                    .py(px(8.))
                    .child(self.game_tile(cx))
                    .child(div().w(px(293.75)).flex_none().child(details)),
            )
            .child(div().h(px(1.)).flex_none().bg(rgba(0x97979f29)))
            .child(
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
                                    .id("dismiss-suggestions")
                                    .cursor_pointer()
                                    .child(icon("x", 18., theme::TEXT_MUTED)),
                            ),
                    )
                    .child(covers),
            );
        if self.browse_open {
            card = card.child(self.popover(cx));
        }
        card
    }
}
