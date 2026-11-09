//! The widget library the Edit Profile board lists, as the live client draws
//! it (captured 2026-11-09, 1920 x 1080, dark theme).

use gpui_kit::{
    AnyElement, App, ClickEvent, Entity, FontWeight, ImageSource, InteractiveElement as _,
    IntoElement, ParentElement as _, StatefulInteractiveElement as _, Styled as _, StyledImage as _,
    Window, div, img, prelude::FluentBuilder as _, px, rgb, rgba,
};

/// A click handler the picker's cards can run.
pub type ClickRun = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

use crate::{icons::icon, media::Media, theme};

/// The faint wash the live cards use for their background and placeholder
/// shapes.
const WASH: u32 = 0x97979f1f;

/// One card in the widget library.
struct WidgetOption {
    label: &'static str,
    /// The glyph over the label: plus in a circle for a new widget, a link for
    /// an account that has to be connected first.
    glyph: &'static str,
    /// The 16 px icon beside the label, when the widget belongs to a game.
    icon: Option<&'static str>,
    /// A small glyph before the label, as the personal widget has.
    title_glyph: Option<&'static str>,
    beta: bool,
}

/// The library the live client lists on 2026-11-09.
const OPTIONS: &[WidgetOption] = &[
    WidgetOption {
        label: "Create your own",
        glyph: "circle-plus",
        icon: None,
        title_glyph: Some("mic"),
        beta: true,
    },
    WidgetOption {
        label: "Marvel Rivals",
        glyph: "link",
        icon: Some("https://cdn.discordapp.com/app-icons/1346069614634864772/2f7be4ce3fb3261f76e544c56709a6ad.png?size=16&keep_aspect_ratio=false"),
        title_glyph: None,
        beta: false,
    },
    WidgetOption {
        label: "Wolvesville",
        glyph: "link",
        icon: Some("https://cdn.discordapp.com/app-icons/672188101707169852/b7a64ea9edb7cb22588dd6530bd66c35.png?size=16&keep_aspect_ratio=false"),
        title_glyph: None,
        beta: false,
    },
    WidgetOption {
        label: "Wuthering Waves",
        glyph: "link",
        icon: Some("https://cdn.discordapp.com/app-icons/1323482066758930452/0a424370271f81fa7399c0938966aadd.png?size=16&keep_aspect_ratio=false"),
        title_glyph: None,
        beta: false,
    },
    WidgetOption {
        label: "Heartopia",
        glyph: "link",
        icon: Some("https://cdn.discordapp.com/app-icons/1463856624455254103/a7bd13821ede07bf4df788e48609d59e.png?size=16&keep_aspect_ratio=false"),
        title_glyph: None,
        beta: false,
    },
    WidgetOption {
        label: "Arknights Endfield",
        glyph: "link",
        icon: Some("https://cdn.discordapp.com/app-icons/1496037472226906183/f7877b08efb704adc7f7992af7e3b799.png?size=16&keep_aspect_ratio=false"),
        title_glyph: None,
        beta: false,
    },
    WidgetOption {
        label: "Neverness to Everness",
        glyph: "link",
        icon: Some("https://cdn.discordapp.com/app-icons/1481180707966685245/8751bdb4c480ed61d49f9ff6d0d471f8.png?size=16&keep_aspect_ratio=false"),
        title_glyph: None,
        beta: false,
    },
    WidgetOption {
        label: "Favorite Game",
        glyph: "circle-plus",
        icon: None,
        title_glyph: None,
        beta: false,
    },
    WidgetOption {
        label: "Games I Like",
        glyph: "circle-plus",
        icon: None,
        title_glyph: None,
        beta: false,
    },
    WidgetOption {
        label: "Games in Rotation",
        glyph: "circle-plus",
        icon: None,
        title_glyph: None,
        beta: false,
    },
    WidgetOption {
        label: "Want to Play",
        glyph: "circle-plus",
        icon: None,
        title_glyph: None,
        beta: false,
    },
];

fn card(
    option: &WidgetOption,
    media: &Entity<Media>,
    on_click: Option<ClickRun>,
    cx: &mut App,
) -> AnyElement {
    let title = div()
        .h(px(20.))
        .flex()
        .items_center()
        .gap(px(4.))
        .when_some(option.icon, |d, url| match media.update(cx, |m, _| m.get(url)) {
            Some(image) => d.child(
                img(ImageSource::Image(image))
                    .size(px(16.))
                    .rounded(px(2.))
                    .object_fit(gpui_kit::ObjectFit::Cover),
            ),
            None => d,
        })
        .when_some(option.title_glyph, |d, glyph| d.child(icon(glyph, 16., theme::TEXT_BRIGHT)))
        .child(
            div()
                .text_size(px(14.))
                .line_height(px(20.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(rgb(theme::TEXT_BRIGHT))
                .child(option.label),
        );

    let body = div()
        .flex_1()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(8.))
        .child(icon(option.glyph, 24., theme::TEXT_BRIGHT))
        .child(title);

    let mut card = div()
        .id(option.label)
        .w(px(207.5))
        .h(px(100.))
        .flex_none()
        .rounded(px(8.))
        .bg(rgba(WASH))
        .cursor_pointer()
        .flex()
        .flex_col()
        .p(px(8.))
        .when_some(on_click, |d, run| d.on_click(run));
    if option.beta {
        card = card.child(div().h(px(16.)).flex_none().flex().justify_end().child(
            div()
                .h(px(16.))
                .flex()
                .items_center()
                .px(px(8.))
                .rounded_full()
                .bg(rgb(theme::ACCENT))
                .text_size(px(12.))
                .line_height(px(16.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(0xffff_ffff))
                .child("Beta"),
        ));
    }
    card.child(body).into_any_element()
}

/// The board's empty state: the widget library, as the live client draws it.
pub fn widget_picker(
    media: &Entity<Media>,
    on_favorite: Option<ClickRun>,
    on_like: Option<ClickRun>,
    cx: &mut App,
) -> AnyElement {
    let mut on_favorite = on_favorite;
    let mut on_like = on_like;
    let mut cards = Vec::new();
    for option in OPTIONS {
        let click = match option.label {
            "Favorite Game" => on_favorite.take(),
            "Games I Like" => on_like.take(),
            _ => None,
        };
        cards.push(card(option, media, click, cx));
    }
    let mut rest = cards.into_iter();
    let mut rows = Vec::new();
    while let Some(first) = rest.next() {
        let second = rest.next();
        let last = second.is_none();
        let mut row = div()
            .flex()
            .gap(px(16.))
            .when(last, |d| d.justify_center())
            .child(first);
        if let Some(second) = second {
            row = row.child(second);
        }
        rows.push(row);
    }

    div()
        .flex()
        .flex_col()
        .items_center()
        .child(
            div()
                .w(px(320.))
                .text_center()
                .text_size(px(16.))
                .line_height(px(20.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(rgb(theme::TEXT_BRIGHT))
                .child("Customize your profile with Widgets"),
        )
        .child(
            div()
                .mt(px(8.))
                .w(px(320.))
                .text_center()
                .text_size(px(14.))
                .line_height(px(18.))
                .text_color(rgb(theme::TEXT_BRIGHT))
                .child("Choose from our library of Widgets to share more about yourself and your interests"),
        )
        .child(div().mt(px(24.)).w(px(431.)).flex().flex_col().gap(px(16.)).children(rows))
        .into_any_element()
}
