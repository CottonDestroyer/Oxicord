//! The Favorite Game widget editor the Edit Profile board shows inline, as
//! the live client draws it (captured 2026-11-09, 1920 x 1080, dark theme).

use gpui_kit::{
    AnyElement, App, Entity, FontWeight, ImageSource, InteractiveElement as _, IntoElement,
    ParentElement as _, Styled as _, StyledImage as _, div, img, px, rgb, rgba,
};

use crate::{icons::icon, media::Media, theme};

/// The faint wash the add tile and the empty covers use.
const WASH: u32 = 0x97979f1f;

struct Suggestion {
    name: &'static str,
    cover: &'static str,
}

/// The games the live editor suggested on 2026-11-09.
const SUGGESTIONS: &[Suggestion] = &[
    Suggestion {
        name: "Lethal Company",
        cover: "https://images-ext-1.discordapp.net/external/kxdIMrNToiGKbJccIo93PX6wHpdMHbkms0kwiOWl6Bs/https/images.igdb.com/igdb/image/upload/t_cover_big/co5ive.jpg?keep_aspect_ratio=true&format=webp",
    },
    Suggestion {
        name: "Overwatch",
        cover: "https://cdn.discordapp.com/app-icons/356875221078245376/843a3b07639f068fdacf40b9c3808c46.webp?size=128&keep_aspect_ratio=true",
    },
    Suggestion {
        name: "Stardew Valley",
        cover: "https://cdn.discordapp.com/app-icons/359509387670192128/2727f218e06d9c3c4204fed086181a53.webp?size=128&keep_aspect_ratio=true",
    },
    Suggestion {
        name: "Destiny 2",
        cover: "https://images-ext-1.discordapp.net/external/KRe_8xMkBNXe0t5RS8_aQc5WqI4Rgl2Sypdf_VLfJVk/https/images.igdb.com/igdb/image/upload/t_cover_big/coc89g.jpg?keep_aspect_ratio=true&format=webp",
    },
    Suggestion {
        name: "Among Us",
        cover: "https://images-ext-1.discordapp.net/external/vwjJ-pK7-GEXq_yHs8oOoJ5Zplu_7VP8dZIZEnW6jLQ/https/images.igdb.com/igdb/image/upload/t_cover_big/co6kqt.jpg?keep_aspect_ratio=true&format=webp",
    },
    Suggestion {
        name: "Escape from Tarkov: Arena",
        cover: "https://images-ext-1.discordapp.net/external/izydqalQpGYCOoUOURoSyS7t-RsETqZ7AWuLsUYv48M/https/images.igdb.com/igdb/image/upload/t_cover_big/cocxnk.jpg?keep_aspect_ratio=true&format=webp",
    },
];

/// The editor, laid out exactly as the live board draws it.
pub fn editor(media: &Entity<Media>, cx: &mut App) -> AnyElement {
    let mut covers = div().flex().gap(px(12.));
    for suggestion in SUGGESTIONS {
        let mut tile = div()
            .id(suggestion.name)
            .w(px(56.2))
            .h(px(74.9))
            .flex_none()
            .rounded(px(8.))
            .overflow_hidden()
            .bg(rgba(WASH))
            .cursor_pointer();
        if let Some(image) = media.update(cx, |m, _| m.get(suggestion.cover)) {
            tile = tile.child(
                img(ImageSource::Image(image)).size_full().object_fit(gpui_kit::ObjectFit::Cover),
            );
        }
        covers = covers.child(tile);
    }

    div()
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
                .child(
                    div()
                        .id("add-game")
                        .w(px(87.25))
                        .h(px(116.33))
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(rgba(WASH))
                        .cursor_pointer()
                        .child(icon("plus", 24., theme::TEXT_MUTED)),
                )
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
        )
        .into_any_element()
}
