//! The Change Avatar Decoration dialog, laid out as the live client draws it
//! (captured 2026-11-09, 1920 x 1080, dark theme).

use gpui_kit::{
    ClickEvent, Context, Entity, EventEmitter, FontWeight, ImageSource, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _,
    Window, div, img, prelude::FluentBuilder as _, px, rgb, rgba,
};

use oxicord_model::{
    collectible::{Decoration, DecorationCatalog},
    Snowflake,
};

use crate::{
    icons::icon,
    media::{Media, avatar},
    profile::modal,
    session::Session,
    theme,
};

/// Emitted when the dialog closes, carrying the decoration that is now applied
/// (its sku, or nothing for None).
pub struct CloseDecorationDialog(pub Option<String>);

impl EventEmitter<CloseDecorationDialog> for DecorationDialog {}

pub struct DecorationDialog {
    session: Entity<Session>,
    media: Entity<Media>,
    catalog: DecorationCatalog,
    draft: Option<String>,
    applied: Option<String>,
}

impl DecorationDialog {
    pub fn new(session: Entity<Session>, media: Entity<Media>) -> Self {
        Self { session, media, catalog: DecorationCatalog::bundled(), draft: None, applied: None }
    }

    pub fn opened(&mut self, applied: Option<String>, cx: &mut Context<Self>) {
        self.draft = applied.clone();
        self.applied = applied;
        cx.notify();
    }

    /// Owned decorations: the catalog's list plus whatever this account is
    /// already wearing, which the live client also lists first.
    fn owned(&self) -> Vec<Decoration> {
        let mut owned = self.catalog.owned.clone();
        if let Some(sku) = &self.applied
            && !owned.iter().any(|d| &d.sku_id == sku)
        {
            let name = self
                .catalog
                .find(sku)
                .map(|d| d.name.clone())
                .unwrap_or_else(|| "Current decoration".to_owned());
            owned.push(Decoration { sku_id: sku.clone(), name, locked: false });
        }
        owned
    }

    fn grid(&self, items: Vec<gpui_kit::AnyElement>) -> gpui_kit::Div {
        div().w(px(250.)).flex().flex_wrap().gap(px(12.)).children(items)
    }

    /// One 75.33 by 80 tile: the artwork over its 12 px label, with the
    /// optional round badge hanging off the top right.
    fn item(
        &self,
        key: impl Into<gpui_kit::ElementId>,
        artwork: gpui_kit::AnyElement,
        label: &str,
        selected: bool,
        badge: Option<(&'static str, f32)>,
        on_click: Option<ClickRun>,
    ) -> gpui_kit::AnyElement {
        let mut item = div()
            .id(key)
            .w(px(75.33))
            .h(px(80.))
            .flex_none()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .rounded(px(4.))
            .bg(rgb(theme::BG_RAIL))
            .cursor_pointer()
            .text_size(px(12.))
            .line_height(px(16.))
            .text_color(rgb(theme::TEXT_BRIGHT))
            .child(artwork)
            .when(!label.is_empty(), |d| d.child(label.to_owned()));
        if let Some(run) = on_click {
            item = item.on_click(run);
        }
        if let Some((glyph, size)) = badge {
            item = item.child(
                div()
                    .absolute()
                    .top(px(-6.))
                    .right(px(-4.))
                    .size(px(20.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_full()
                    .bg(rgba(0x0000_0099))
                    .child(icon(glyph, size, theme::TEXT_BRIGHT)),
            );
        }
        if selected {
            item = item.child(
                div()
                    .absolute()
                    .inset_0()
                    .rounded(px(4.))
                    .border(px(2.))
                    .border_color(rgb(theme::ACCENT)),
            );
        }
        item.into_any_element()
    }

    fn decoration_tile(
        &self,
        item: &Decoration,
        selected: bool,
        badge: Option<(&'static str, f32)>,
        cx: &mut Context<Self>,
    ) -> gpui_kit::AnyElement {
        let sku = item.sku_id.clone();
        let artwork = match sku.parse::<Snowflake>().ok().and_then(|sku| {
            self.media.update(cx, |media, _| media.get(&oxicord_media::cdn::collectible(sku)))
        }) {
            Some(image) => div()
                .size(px(64.))
                .child(img(ImageSource::Image(image)).size_full())
                .into_any_element(),
            None => div().size(px(64.)).into_any_element(),
        };
        self.item(
            gpui_kit::ElementId::Name(format!("decoration-{sku}").into()),
            artwork,
            "",
            selected,
            badge,
            (!item.locked).then(|| {
                Box::new(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.draft = Some(sku.clone());
                    cx.notify();
                })) as ClickRun
            }),
        )
    }

    fn preview_avatar(&self, size: f32, sku: Option<&str>, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let me = self.session.read(cx).store.me().cloned();
        let url = me
            .as_ref()
            .map(|m| oxicord_media::cdn::user_avatar(m.id, m.avatar.as_deref(), 128))
            .unwrap_or_default();
        let name = me.as_ref().map(|m| m.display_name().to_owned()).unwrap_or_default();
        let mut boxed = div().relative().size(px(size)).flex_none().child(avatar(&self.media, &url, size, &name, cx));
        if let Some(sku) = sku.and_then(|s| s.parse::<Snowflake>().ok())
            && let Some(frame) = crate::media::decoration(&self.media, sku, size, cx)
        {
            boxed = boxed.child(frame);
        }
        boxed.into_any_element()
    }
}

impl Render for DecorationDialog {
    fn render(&mut self, _: &mut gpui_kit::Window, cx: &mut Context<Self>) -> impl IntoElement {
        let owned = self.owned();
        let owned_skus: Vec<&str> = owned.iter().map(|d| d.sku_id.as_str()).collect();

        let none_selected = self.draft.is_none();
        let none = self.item(
            "decoration-none",
            div().mb(px(4.)).child(icon("slash-circle", 24., theme::TEXT_BRIGHT)).into_any_element(),
            "None",
            none_selected,
            None,
            Some(Box::new(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.draft = None;
                cx.notify();
            }))),
        );
        let shop = self.item(
            "decoration-shop",
            div().mb(px(4.)).child(icon("store", 23., theme::TEXT_BRIGHT)).into_any_element(),
            "Shop",
            false,
            None,
            None,
        );
        let mut your_items = vec![none, shop];
        for item in &owned {
            your_items.push(self.decoration_tile(
                item,
                self.draft.as_deref() == Some(item.sku_id.as_str()),
                None,
                cx,
            ));
        }

        let nitro: Vec<_> = self
            .catalog
            .nitro
            .iter()
            .filter(|d| !owned_skus.contains(&d.sku_id.as_str()))
            .map(|d| self.decoration_tile(d, false, Some(("nitro", 14.)), cx))
            .collect();
        let shop_items: Vec<_> = self
            .catalog
            .shop
            .iter()
            .filter(|d| !owned_skus.contains(&d.sku_id.as_str()))
            .map(|d| self.decoration_tile(d, false, Some(("lock-badge", 12.)), cx))
            .collect();

        let section = |label: &'static str, grid: gpui_kit::Div| {
            div()
                .flex()
                .flex_col()
                .gap(px(4.))
                .pb(px(12.))
                .child(
                    div()
                        .text_size(px(16.))
                        .line_height(px(20.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(theme::TEXT_BRIGHT))
                        .child(label),
                )
                .child(grid)
        };

        let list = div()
            .id("decoration-list")
            .ml(px(-12.))
            .w(px(274.))
            .h(px(420.))
            .flex_none()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap(px(4.))
            .pl(px(12.))
            .pb(px(16.))
            .child(section("Your Decorations", self.grid(your_items)))
            .child(section("Exclusive to Nitro", self.grid(nitro)))
            .child(section("See What's in Shop", self.grid(shop_items)));

        let preview = div()
            .mt(px(24.))
            .w(px(280.))
            .flex_none()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(
                div()
                    .w(px(280.))
                    .h(px(212.))
                    .flex()
                    .border(px(2.))
                    .border_color(rgba(0x0000_0000))
                    .child(
                        div()
                            .w(px(208.))
                            .h(px(208.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(4.))
                            .bg(rgb(theme::BG_RAIL))
                            .child(self.preview_avatar(120., self.draft.as_deref(), cx)),
                    )
                    .child(
                        div()
                            .ml(px(12.))
                            .w(px(56.))
                            .h(px(208.))
                            .flex_none()
                            .flex()
                            .flex_col()
                            .items_center()
                            .rounded(px(4.))
                            .bg(rgb(theme::BG_RAIL))
                            .p(px(8.))
                            .child(self.preview_avatar(40., self.draft.as_deref(), cx))
                            .child(
                                div()
                                    .mt(px(16.))
                                    .child(self.preview_avatar(32., self.draft.as_deref(), cx)),
                            )
                            .child(
                                div()
                                    .mt(px(16.))
                                    .child(self.preview_avatar(40., self.draft.as_deref(), cx)),
                            )
                            .child(
                                div()
                                    .mt(px(16.))
                                    .child(self.preview_avatar(32., self.draft.as_deref(), cx)),
                            ),
                    ),
            );

        let dirty = self.draft != self.applied;
        let footer = div()
            .h(px(72.))
            .flex_none()
            .flex()
            .items_center()
            .justify_end()
            .gap(px(8.))
            .px(px(24.))
            .py(px(16.))
            .child(
                div()
                    .id("decoration-cancel")
                    .w(px(100.))
                    .h(px(40.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(theme::RADIUS))
                    .border_1()
                    .border_color(rgba(0x97979f29))
                    .bg(rgb(theme::SECONDARY))
                    .cursor_pointer()
                    .text_size(px(16.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(theme::TEXT_BRIGHT))
                    .hover(|s| s.bg(rgb(theme::SECONDARY_HOVER)))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        cx.emit(CloseDecorationDialog(this.applied.clone()));
                    }))
                    .child("Cancel"),
            )
            .child(
                div()
                    .id("decoration-apply")
                    .w(px(100.))
                    .h(px(40.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(theme::RADIUS))
                    .border_1()
                    .border_color(rgba(0xffff_ff14))
                    .bg(rgb(theme::ACCENT))
                    .when(!dirty, |d| d.opacity(0.5))
                    .cursor_pointer()
                    .text_size(px(16.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(theme::TEXT_BRIGHT))
                    .hover(|s| s.bg(rgb(theme::ACCENT_HOVER)))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        cx.emit(CloseDecorationDialog(this.draft.clone()));
                    }))
                    .child("Apply"),
            );

        let dialog = div()
            .id("decoration-dialog")
            .occlude()
            .w(px(604.))
            .h(px(542.))
            .flex_none()
            .flex()
            .flex_col()
            .overflow_hidden()
            .rounded(px(theme::RADIUS_MD))
            .bg(rgb(theme::BG_PANEL))
            .border_1()
            .border_color(rgb(theme::BORDER_OVERLAY))
            .child(
                div()
                    .h(px(40.))
                    .flex_none()
                    .flex()
                    .items_start()
                    .justify_between()
                    .pl(px(24.))
                    .pr(px(16.))
                    .pt(px(16.))
                    .child(
                        div()
                            .text_size(px(20.))
                            .line_height(px(24.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(theme::TEXT_BRIGHT))
                            .child("Change Avatar Decoration"),
                    )
                    .child(
                        div()
                            .id("decoration-close")
                            .size(px(24.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(3.))
                            .opacity(0.5)
                            .cursor_pointer()
                            .hover(|s| s.opacity(1.))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                cx.emit(CloseDecorationDialog(this.applied.clone()));
                            }))
                            .child(icon("x", 24., theme::TEXT_BRIGHT)),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .gap(px(12.))
                    .px(px(24.))
                    .pt(px(8.))
                    .child(list)
                    .child(preview),
            )
            .child(footer);

        modal(
            dialog.into_any_element(),
            Box::new(cx.listener(|this, _: &ClickEvent, _, cx| {
                cx.emit(CloseDecorationDialog(this.applied.clone()));
            })),
        )
        .into_any_element()
    }
}

type ClickRun = Box<dyn Fn(&ClickEvent, &mut Window, &mut gpui_kit::App) + 'static>;
