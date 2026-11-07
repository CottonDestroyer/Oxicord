//! The "Send a Gift" dialog reached from the composer's "Send a gift" button.
//! Discord shows two Nitro tiers (Nitro and Nitro Basic) with their perks and a
//! "1 gift away from the Patron Badge!" footer. Oxicord has no payment backend,
//! so this is a faithful read-only view of the upsell — selecting a tier closes
//! the dialog without charging anything, and that is documented as such.

use gpui_kit::{
    ClickEvent, Context, Entity, EventEmitter, FontWeight, InteractiveElement as _, IntoElement, KeyDownEvent,
    ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _, Subscription, Window, div, px, rgb,
};
use gpui_kit::prelude::FluentBuilder as _;

use crate::{icons::icon, profile::modal, session::Session, theme};

pub struct CloseGift;

impl EventEmitter<CloseGift> for GiftDialog {}

struct Tier {
    name: &'static str,
    badge: Option<&'static str>,
    monthly: Option<&'static str>,
    yearly: Option<&'static str>,
    perks: &'static [&'static str],
    featured: bool,
}

const TIERS: &[Tier] = &[
    Tier {
        name: "Nitro",
        badge: Some("2-WEEK TRIAL"),
        monthly: None,
        yearly: None,
        perks: &[
            "1GB uploads",
            "Custom emoji anywhere",
            "Unlimited Super Reactions",
            "HD video streaming",
            "2 Server Boosts",
            "Custom profiles and more!",
        ],
        featured: true,
    },
    Tier {
        name: "Nitro Basic",
        badge: None,
        monthly: Some("€3.03"),
        yearly: Some("€30.41"),
        perks: &[
            "50MB uploads",
            "Custom emoji anywhere",
            "Unlimited Super Reactions",
            "Special Nitro badge on your profile",
        ],
        featured: false,
    },
];

pub struct GiftDialog {
    /// Retained so the dialog re-renders if the session state changes.
    _session: Entity<Session>,
    _subscriptions: Vec<Subscription>,
}

impl GiftDialog {
    pub fn new(session: Entity<Session>, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        let subscriptions = vec![cx.observe(&session, |_, _, cx| cx.notify())];
        Self { _session: session, _subscriptions: subscriptions }
    }

    /// No-op acknowledgement that a tier was chosen. Payment is not implemented.
    fn choose(&mut self, cx: &mut Context<Self>) {
        cx.emit(CloseGift);
    }
}

impl Render for GiftDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let card = div()
            .id("gift-card")
            .occlude()
            .w(px(520.))
            .p(px(16.))
            .flex()
            .flex_col()
            .gap(px(16.))
            .rounded(px(theme::RADIUS))
            .border_1()
            .border_color(rgb(theme::BORDER_MUTED))
            .bg(rgb(theme::BG_PANEL))
            .capture_key_down(cx.listener(|_, event: &KeyDownEvent, _, cx| {
                if event.keystroke.key == "escape" {
                    cx.emit(CloseGift);
                    cx.stop_propagation();
                }
            }))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(20.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(theme::HEADER_PRIMARY))
                            .child("Send a Gift"),
                    )
                    .child(
                        div()
                            .id("gift-close")
                            .cursor_pointer()
                            .text_color(rgb(theme::TEXT_MUTED))
                            .on_click(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(CloseGift)))
                            .child("✕"),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap(px(12.))
                    .children(TIERS.iter().enumerate().map(|(idx, tier)| {
                        let accent = rgb(if tier.featured { theme::ACCENT } else { theme::BORDER_MUTED });
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .gap(px(10.))
                            .p(px(14.))
                            .rounded(px(theme::RADIUS))
                            .border_1()
                            .border_color(accent)
                            .bg(rgb(theme::BG_HOVER))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .child(
                                        div()
                                            .text_size(px(16.))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(rgb(theme::TEXT))
                                            .child(tier.name),
                                    )
                                    .when_some(tier.badge, |row, text| {
                                        row.child(
                                            div()
                                                .px(px(6.))
                                                .py(px(2.))
                                                .rounded_full()
                                                .bg(rgb(theme::ACCENT))
                                                .text_size(px(10.))
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(rgb(0xffffff))
                                                .child(text),
                                        )
                                    }),
                            )
                            .when_some(tier.monthly, |col, m| {
                                col.child(
                                    div()
                                        .flex()
                                        .items_end()
                                        .gap(px(2.))
                                        .child(
                                            div()
                                                .text_size(px(22.))
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(rgb(theme::TEXT_BRIGHT))
                                                .child(m),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(12.))
                                                .text_color(rgb(theme::TEXT_MUTED))
                                                .child("/month"),
                                        ),
                                )
                                .child(
                                    div()
                                        .text_size(px(12.))
                                        .text_color(rgb(theme::TEXT_MUTED))
                                        .child(format!("{} /year", tier.yearly.unwrap_or(""))),
                                )
                            })
                            .child(
                                div()
                                    .h(px(1.))
                                    .bg(rgb(theme::BORDER_MUTED)),
                            )
                            .children(tier.perks.iter().map(|perk| {
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.))
                                    .child(icon("check", 14., theme::ACCENT))
                                    .child(
                                        div()
                                            .text_size(px(13.))
                                            .text_color(rgb(theme::TEXT))
                                            .child(*perk),
                                    )
                            }))
                            .child(
                                div()
                                    .id(("gift-choose", idx as u64))
                                    .h(px(36.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded(px(theme::RADIUS))
                                    .cursor_pointer()
                                    .text_size(px(14.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(rgb(0xffffff))
                                    .bg(rgb(theme::ACCENT))
                                    .hover(|s| s.opacity(0.85))
                                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.choose(cx)))
                                    .child(format!("Gift {}", tier.name)),
                            )
                    })),
            )
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(rgb(theme::TEXT_MUTED))
                    .child("1 gift away from the Patron Badge!"),
            );

        modal(card.into_any_element(), Box::new(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(CloseGift))))
    }
}
