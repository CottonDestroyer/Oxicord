//! The pages behind the Nitro, Shop and Quests shortcuts under Friends. The
//! official client fills them from its store catalog. Here they are read only
//! shells: the page chrome as drawn live, and no purchase or catalog flow.

use gpui_kit::{
    AnyElement, FontWeight, IntoElement, ParentElement as _, Styled as _, div, linear_color_stop, linear_gradient,
    prelude::FluentBuilder as _, px, rgb, rgba,
};

use crate::{icons::icon, theme};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Nitro,
    Shop,
    Quests,
}

impl Page {
    pub fn label(self) -> &'static str {
        match self {
            Page::Nitro => "Nitro",
            Page::Shop => "Shop",
            Page::Quests => "Quests",
        }
    }

    /// The icon file, also the name the shortcut is remembered by.
    pub fn glyph(self) -> &'static str {
        match self {
            Page::Nitro => "nitro",
            Page::Shop => "store",
            Page::Quests => "quests",
        }
    }

    /// Nitro and Shop wear a NEW pill until they are first opened.
    pub fn has_new_pill(self) -> bool {
        matches!(self, Page::Nitro | Page::Shop)
    }

    pub fn key(self) -> &'static str {
        match self {
            Page::Nitro => "nitro",
            Page::Shop => "shop",
            Page::Quests => "quests",
        }
    }
}

pub fn page(which: Page) -> AnyElement {
    match which {
        Page::Nitro => nitro(),
        Page::Shop => shop(),
        Page::Quests => quests(),
    }
}

/// The strip across the top of the Nitro and Shop pages: 49px high, the
/// page's icon at the left and its section links after it.
fn nav(glyph: &str, links: &[(&'static str, bool)]) -> gpui_kit::Div {
    div()
        .h(px(49.))
        .flex_none()
        .flex()
        .items_center()
        .gap(px(20.))
        .px(px(16.))
        .child(div().w(px(24.)).flex_none().child(icon(glyph, 24., 0xffffff)))
        .children(links.iter().map(|&(label, selected)| {
            div()
                .h_full()
                .flex()
                .items_center()
                .relative()
                .text_size(px(14.))
                .line_height(px(18.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(rgb(if selected { 0xfcfcfc } else { 0xe3e3e6 }))
                .child(label)
                .when(selected, |s| {
                    s.child(div().absolute().left_0().right_0().bottom(px(0.)).h(px(2.)).bg(rgb(0xffffff)))
                })
        }))
}

fn pill_button(label: &'static str, fill: u32, text: u32, glyph: Option<&str>, height: f32) -> gpui_kit::Div {
    div()
        .h(px(height))
        .px(px(16.))
        .flex()
        .items_center()
        .justify_center()
        .gap(px(6.))
        .rounded(px(8.))
        .bg(rgba(fill))
        .text_size(px(14.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(rgb(text))
        .children(glyph.map(|g| icon(g, 16., text)))
        .child(label)
}

/// What the page says in place of the store catalog.
fn read_only_note() -> gpui_kit::Div {
    div()
        .text_size(px(12.))
        .line_height(px(16.))
        .text_color(rgba(0xffffff99))
        .child("Purchases are not available in Oxicord.")
}

fn nitro() -> AnyElement {
    div()
        .flex_1()
        .min_w_0()
        .h_full()
        .flex()
        .flex_col()
        .bg(linear_gradient(
            180.,
            linear_color_stop(rgb(0xa842d3), 0.),
            linear_color_stop(rgb(0x1b1230), 0.62),
        ))
        .child(
            nav(
                "nitro",
                &[("Home", true), ("What's New", false), ("Best of Nitro", false), ("Plans", false), ("Compare", false)],
            )
            .child(div().flex_1())
            .child(pill_button("Gift Nitro", 0x0000008f, 0xffffff, None, 32.)),
        )
        .child(
            div()
                .flex_1()
                .min_h_0()
                .flex()
                .flex_col()
                .items_center()
                .pt(px(70.))
                .child(
                    div()
                        .w(px(800.))
                        .text_center()
                        .text_size(px(64.))
                        .line_height(px(58.))
                        .font_weight(FontWeight::BLACK)
                        .italic()
                        .text_color(rgb(0xffffff))
                        .child("UNLOCK A WORLD OF PERKS WITH NITRO"),
                )
                .child(
                    div()
                        .mt(px(24.))
                        .flex()
                        .gap(px(16.))
                        .child(pill_button("Unlock with Nitro", 0xffffffff, 0x000000, Some("nitro"), 40.).w(px(177.)))
                        .child(pill_button("Gift Nitro", 0x97979f1f, 0xfbfbfb, None, 40.).w(px(121.))),
                )
                .child(div().mt(px(24.)).child(read_only_note())),
        )
        .into_any_element()
}

fn shop() -> AnyElement {
    div()
        .flex_1()
        .min_w_0()
        .h_full()
        .flex()
        .flex_col()
        .bg(rgb(theme::BG_CHAT))
        .child(
            nav(
                "store",
                &[
                    ("Featured", true),
                    ("Browse", false),
                    ("Orbs Shop", false),
                    ("Game Shops", false),
                    ("Game Servers", false),
                ],
            )
            .child(div().flex_1())
            .child(
                div()
                    .w(px(180.))
                    .h(px(32.))
                    .px(px(10.))
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .rounded(px(8.))
                    .bg(rgb(theme::BG_PANEL))
                    .text_size(px(14.))
                    .text_color(rgb(theme::TEXT_MUTED))
                    .child(icon("search", 14., theme::TEXT_MUTED))
                    .child("Search the Shop"),
            ),
        )
        .child(
            div()
                .flex_1()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(8.))
                .child(
                    div()
                        .text_size(px(20.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(theme::TEXT_BRIGHT))
                        .child("The Shop is not available here"),
                )
                .child(
                    div()
                        .text_size(px(14.))
                        .text_color(rgb(theme::TEXT_MUTED))
                        .child("Profile decorations and effects can be picked in your profile settings."),
                ),
        )
        .into_any_element()
}

/// The official client hands Quests to the desktop app, and so does this.
fn quests() -> AnyElement {
    div()
        .flex_1()
        .min_w_0()
        .h_full()
        .flex()
        .items_center()
        .justify_center()
        .bg(linear_gradient(
            135.,
            linear_color_stop(rgb(0x1a0b5a), 0.),
            linear_color_stop(rgb(0x1f5fe0), 0.7),
        ))
        .child(
            div()
                .w(px(480.))
                .h(px(296.))
                .pt(px(36.))
                .flex()
                .flex_col()
                .items_center()
                .gap(px(40.))
                .rounded(px(8.))
                .bg(rgb(0x262629))
                .child(
                    div()
                        .text_size(px(20.))
                        .line_height(px(24.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(0xffffff))
                        .child("Opening Discord App."),
                )
                .child(icon("discord-mark", 72., 0xffffff)),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_nitro_and_shop_wear_the_new_pill() {
        assert!(Page::Nitro.has_new_pill());
        assert!(Page::Shop.has_new_pill());
        assert!(!Page::Quests.has_new_pill());
    }

    #[test]
    fn each_page_is_remembered_under_its_own_name() {
        let keys = [Page::Nitro.key(), Page::Shop.key(), Page::Quests.key()];
        assert_eq!(keys, ["nitro", "shop", "quests"]);
    }
}
