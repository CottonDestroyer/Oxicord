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

/// The border colour the cards, chips and menus share.
const LINE: u32 = 0x97979f33;

struct Game {
    name: &'static str,
    /// The game's Discord application id, as the save request names it.
    id: &'static str,
    cover: Option<&'static str>,
}

/// The games the live browse list and suggestions contained on 2026-11-09,
/// with the ids read from that list.
const GAMES: &[Game] = &[
    Game {
        name: "Lethal Company",
        id: "1167674267748540516",
        cover: Some("https://images-ext-1.discordapp.net/external/kxdIMrNToiGKbJccIo93PX6wHpdMHbkms0kwiOWl6Bs/https/images.igdb.com/igdb/image/upload/t_cover_big/co5ive.jpg?keep_aspect_ratio=true&format=webp"),
    },
    Game {
        name: "Overwatch",
        id: "356875221078245376",
        cover: Some("https://cdn.discordapp.com/app-icons/356875221078245376/843a3b07639f068fdacf40b9c3808c46.webp?size=128&keep_aspect_ratio=true"),
    },
    Game {
        name: "Stardew Valley",
        id: "359509387670192128",
        cover: Some("https://cdn.discordapp.com/app-icons/359509387670192128/2727f218e06d9c3c4204fed086181a53.webp?size=128&keep_aspect_ratio=true"),
    },
    Game {
        name: "Destiny 2",
        id: "372438022647578634",
        cover: Some("https://images-ext-1.discordapp.net/external/KRe_8xMkBNXe0t5RS8_aQc5WqI4Rgl2Sypdf_VLfJVk/https/images.igdb.com/igdb/image/upload/t_cover_big/coc89g.jpg?keep_aspect_ratio=true&format=webp"),
    },
    Game {
        name: "Among Us",
        id: "1402418440685486130",
        cover: Some("https://images-ext-1.discordapp.net/external/vwjJ-pK7-GEXq_yHs8oOoJ5Zplu_7VP8dZIZEnW6jLQ/https/images.igdb.com/igdb/image/upload/t_cover_big/co6kqt.jpg?keep_aspect_ratio=true&format=webp"),
    },
    Game {
        name: "Escape from Tarkov: Arena",
        id: "1215361187684946010",
        cover: Some("https://images-ext-1.discordapp.net/external/izydqalQpGYCOoUOURoSyS7t-RsETqZ7AWuLsUYv48M/https/images.igdb.com/igdb/image/upload/t_cover_big/cocxnk.jpg?keep_aspect_ratio=true&format=webp"),
    },
    Game { name: "ROBLOX", id: "363445589247131668", cover: None },
    Game { name: "VALORANT", id: "700136079562375258", cover: None },
    Game { name: "League of Legends", id: "1402418696126992445", cover: None },
    Game { name: "Minecraft", id: "1402418491272986635", cover: None },
    Game { name: "Counter-Strike 2", id: "1158877933042143272", cover: None },
    Game { name: "Fortnite", id: "1402418703554842694", cover: None },
    Game { name: "Grand Theft Auto V Legacy", id: "1402418714716143646", cover: None },
    Game { name: "Dota 2", id: "356875988589740042", cover: None },
    Game { name: "Rocket League", id: "356877880938070016", cover: None },
    Game { name: "Call of Duty®: MWIII", id: "1402418487745581248", cover: None },
    Game { name: "PUBG: BATTLEGROUNDS", id: "1402418717781921935", cover: None },
    Game { name: "Apex Legends", id: "542075586886107149", cover: None },
    Game { name: "Genshin Impact", id: "762434991303950386", cover: None },
    Game { name: "ELDEN RING", id: "1402418436809953330", cover: None },
];

/// The suggested covers, which are the first six games.
const SUGGESTED: usize = 6;

/// The tag menu's groups, their labels and the wire value each sends.
/// The empty wire value is the "None" option, which sends no tag.
const TAG_GROUPS: &[(&str, &[(&str, &str)])] = &[
    (
        "Skill level",
        &[
            ("None", ""),
            ("Casual", "casual"),
            ("Intermediate", "intermediate"),
            ("Expert", "expert"),
            ("Better than you", "better_than_you"),
        ],
    ),
    (
        "Rating",
        &[
            ("Obsessed", "obsessed"),
            ("Love it", "love_it"),
            ("Kind of love it", "kind_of_love_it"),
            ("Kind of hate it", "kind_of_hate_it"),
            ("Rage quitting", "rage_quitting"),
        ],
    ),
    (
        "Looking for",
        &[
            ("Looking for group", "looking_for_group"),
            ("Open to play", "open_to_play"),
            ("Looking for tips", "looking_for_tips"),
            ("Open to teach", "open_to_teach"),
            ("Looking to discuss", "looking_to_discuss"),
        ],
    ),
];

/// The game, comment and tags a pending Favorite Game widget holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedFavorite {
    pub name: &'static str,
    pub cover: Option<&'static str>,
    pub comment: String,
    pub tags: Vec<&'static str>,
}

pub struct FavoriteGameEditor {
    media: Entity<Media>,
    search: Entity<InputState>,
    comment: Entity<InputState>,
    browse_open: bool,
    tags_open: bool,
    selected: Option<usize>,
    /// One tag per group, as the group and the chosen value.
    tags: Vec<(usize, usize)>,
    _subscriptions: Vec<Subscription>,
}

impl FavoriteGameEditor {
    pub fn new(media: Entity<Media>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search"));
        let comment = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Let everyone know why this is your favorite")
        });
        let subscriptions = vec![
            cx.subscribe(&search, |_, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            }),
            cx.subscribe(&comment, |_, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            }),
        ];
        Self {
            media,
            search,
            comment,
            browse_open: false,
            tags_open: false,
            selected: None,
            tags: Vec::new(),
            _subscriptions: subscriptions,
        }
    }

    /// A fresh editor, as the live client shows it when the card is picked.
    pub fn opened(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.browse_open = false;
        self.tags_open = false;
        self.selected = None;
        self.tags.clear();
        self.comment.update(cx, |state, cx| state.set_value("", window, cx));
        cx.notify();
    }

    fn pick(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.selected = Some(index);
        self.browse_open = false;
        self.tags_open = false;
        self.tags.clear();
        self.comment.update(cx, |state, cx| state.set_value("", window, cx));
        cx.notify();
    }

    /// What the board draws for the chosen game, if one is chosen.
    pub fn saved(&self, cx: &gpui_kit::App) -> Option<SavedFavorite> {
        let game = self.selected.map(|index| &GAMES[index])?;
        Some(SavedFavorite {
            name: game.name,
            cover: game.cover,
            comment: self.comment.read(cx).value().trim().to_owned(),
            tags: self.tags.iter().map(|&(group, value)| TAG_GROUPS[group].1[value].0).collect(),
        })
    }

    /// The `PUT /users/@me/widgets` entry for the chosen game. Built here so
    /// the shape is exercised, but never sent from the real account.
    pub fn submission(&self, cx: &gpui_kit::App) -> Option<oxicord_model::WidgetSubmission> {
        let saved = self.saved(cx)?;
        Some(oxicord_model::WidgetSubmission {
            id: None,
            data: oxicord_model::WidgetDataSubmission::FavoriteGames {
                games: vec![oxicord_model::GameEntrySubmission {
                    game_id: GAMES[self.selected?].id.parse().ok()?,
                    comment: saved.comment,
                    tags: self
                        .tags
                        .iter()
                        .filter_map(|&(group, value)| {
                            let wire = TAG_GROUPS[group].1[value].1;
                            (!wire.is_empty()).then(|| wire.to_owned())
                        })
                        .collect(),
                }],
            },
        })
    }

    fn game_tile(&self, cx: &mut Context<Self>) -> AnyElement {
        match self.selected.and_then(|index| GAMES[index].cover) {
            Some(url) => match self.media.update(cx, |m, _| m.get(url)) {
                Some(image) => img(ImageSource::Image(image))
                    .w(px(87.25))
                    .h(px(116.33))
                    .flex_none()
                    .rounded(px(8.))
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
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        this.pick(index, window, cx)
                    }))
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

    /// One chosen tag, as the live chip row draws it.
    fn chip(&self, group: usize, value: usize, cx: &mut Context<Self>) -> AnyElement {
        let label = TAG_GROUPS[group].1[value].0;
        div()
            .id(gpui_kit::ElementId::Name(format!("tag-{group}-{value}").into()))
            .h(px(20.))
            .flex_none()
            .flex()
            .items_center()
            .gap(px(4.))
            .rounded(px(8.))
            .border_1()
            .border_color(rgba(0x97979f33))
            .pl(px(6.))
            .pr(px(8.))
            .py(px(3.))
            .child(icon("tag-award", 12., theme::TEXT_MUTED))
            .child(
                div()
                    .text_size(px(10.))
                    .line_height(px(12.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(theme::TEXT_MUTED))
                    .child(label),
            )
            .child(
                div()
                    .id(gpui_kit::ElementId::Name(format!("untag-{group}-{value}").into()))
                    .flex_none()
                    .p(px(3.))
                    .ml(px(-1.))
                    .mr(px(-3.))
                    .my(px(-3.))
                    .rounded(px(4.))
                    .cursor_pointer()
                    .hover(|d| d.bg(rgba(WASH)))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.tags.retain(|&(g, v)| !(g == group && v == value));
                        cx.notify();
                    }))
                    .child(icon("x", 12., theme::TEXT_MUTED)),
            )
            .into_any_element()
    }

    /// The tags menu, anchored to the right of the add tag chip.
    fn tags_menu(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut groups = div().flex().flex_col().w_full().gap(px(17.));
        for (group, (label, values)) in TAG_GROUPS.iter().enumerate() {
            let mut block = div().flex().flex_col().child(
                div()
                    .h(px(36.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .px(px(8.))
                    .text_size(px(14.))
                    .line_height(px(20.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(theme::TEXT_MUTED))
                    .child(*label),
            );
            for (value, (name, _)) in values.iter().enumerate() {
                let chosen = self.tags.contains(&(group, value));
                let radio = div()
                    .size(px(20.))
                    .flex_none()
                    .rounded_full()
                    .when(chosen, |d| d.bg(rgb(theme::ACCENT)).child(
                        div().size(px(8.)).rounded_full().bg(rgb(0xffff_ffff)),
                    ))
                    .when(!chosen, |d| {
                        d.bg(rgba(0x00000014)).border_1().border_color(rgba(0x97979fa3))
                    })
                    .flex()
                    .items_center()
                    .justify_center();
                block = block.child(
                    div()
                        .id(gpui_kit::ElementId::Name(
                            format!("tag-option-{group}-{value}").into(),
                        ))
                        .h(px(36.))
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_between()
                        .px(px(8.))
                        .rounded(px(2.))
                        .cursor_pointer()
                        .text_size(px(14.))
                        .line_height(px(18.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(rgb(theme::TEXT_BRIGHT))
                        .hover(|d| d.bg(rgba(WASH)))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            match this.tags.iter().position(|&(g, _)| g == group) {
                                Some(index) => {
                                    if this.tags[index].1 == value {
                                        this.tags.remove(index);
                                    } else {
                                        this.tags[index].1 = value;
                                    }
                                }
                                None => this.tags.push((group, value)),
                            }
                            cx.notify();
                        }))
                        .child(*name)
                        .child(radio),
                );
            }
            groups = groups.child(block);
        }

        div()
            .id("tags-menu")
            .absolute()
            .left(px(50.4))
            .top_0()
            .w(px(172.7))
            .h(px(400.))
            .rounded(px(8.))
            .border_1()
            .border_color(rgba(LINE))
            .bg(rgb(theme::BG_CARD))
            .overflow_y_scroll()
            .child(div().flex().flex_col().p(px(8.)).child(groups))
            .into_any_element()
    }

    /// The comment field and tag chips under the chosen game.
    fn details(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut tags = div().mt(px(4.)).h(px(20.)).flex_none().flex().items_center().gap(px(4.));
        for &(group, value) in &self.tags {
            tags = tags.child(self.chip(group, value, cx));
        }
        tags = tags.child(
            div()
                .relative()
                .h(px(20.))
                .w(px(42.4))
                .flex_none()
                .child(
                    div()
                        .id("add-tags")
                        .h(px(20.))
                        .flex_none()
                        .flex()
                        .items_center()
                        .rounded(px(8.))
                        .border_1()
                        .border_dashed()
                        .border_color(rgba(0x97979f33))
                        .pl(px(6.))
                        .pr(px(8.))
                        .py(px(3.))
                        .cursor_pointer()
                        .text_size(px(10.))
                        .line_height(px(12.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(rgb(theme::TEXT_MUTED))
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.tags_open = !this.tags_open;
                            cx.notify();
                        }))
                        .child("+ Tags"),
                )
                .when(self.tags_open, |d| d.child(self.tags_menu(cx))),
        );

        div()
            .w(px(293.75))
            .flex_none()
            .flex()
            .flex_col()
            .gap(px(4.))
            .justify_center()
            .child(
                div()
                    .text_size(px(14.))
                    .line_height(px(18.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(theme::TEXT_BRIGHT))
                    .child(GAMES[self.selected.unwrap_or(0)].name),
            )
            .child(
                div()
                    .ml(px(-7.))
                    .pl(px(7.))
                    .w(px(300.75))
                    .flex_none()
                    .child(Input::new(&self.comment).appearance(false).bordered(false)),
            )
            .child(tags)
            .into_any_element()
    }
}

impl Render for FavoriteGameEditor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let selected = self.selected.is_some();

        let header = div()
            .h(px(if selected { 34. } else { 18. }))
            .flex_none()
            .flex()
            .flex_col()
            .justify_center()
            .child(
                div()
                    .text_size(px(14.))
                    .line_height(px(18.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(theme::TEXT_BRIGHT))
                    .child("Favorite Game"),
            )
            .when(selected, |d| {
                d.child(
                    div()
                        .text_size(px(12.))
                        .line_height(px(16.))
                        .text_color(rgb(theme::TEXT_MUTED))
                        .child("Choose 1 game"),
                )
            });

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
            .child(header);

        if selected {
            card = card.child(
                div()
                    .relative()
                    .h(px(116.33))
                    .flex_none()
                    .flex()
                    .gap(px(16.))
                    .child(self.game_tile(cx))
                    .child(self.details(cx))
                    .child(
                        div()
                            .id("remove-game")
                            .absolute()
                            .top(px(8.))
                            .right(px(8.))
                            .size(px(32.))
                            .rounded(px(8.))
                            .border_1()
                            .border_color(rgba(0x97979f29))
                            .bg(rgba(0x0000008f))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                this.selected = None;
                                this.tags_open = false;
                                this.tags.clear();
                                this.comment
                                    .update(cx, |state, cx| state.set_value("", window, cx));
                                cx.notify();
                            }))
                            .child(icon("trash", 16., 0xffff_ffff)),
                    ),
            );
        } else {
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
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        this.pick(index, window, cx)
                    }));
                if let Some(url) = game.cover
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

            card = card
                .child(
                    div()
                        .h(px(132.3))
                        .flex_none()
                        .flex()
                        .items_center()
                        .gap(px(16.))
                        .py(px(8.))
                        .child(self.game_tile(cx))
                        .child(
                            div()
                                .w(px(293.75))
                                .flex_none()
                                .text_size(px(12.))
                                .line_height(px(16.))
                                .text_color(rgb(theme::TEXT_MUTED))
                                .child("Add one game. This Widget won't show up on your profile until you add a game."),
                        ),
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
        }

        if self.browse_open {
            card = card.child(self.popover(cx));
        }
        card
    }
}
