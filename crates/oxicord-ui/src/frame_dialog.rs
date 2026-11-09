//! The Change Profile Frame dialog, laid out as the live client draws it
//! (captured 2026-11-09, 1920 x 1080, dark theme).

use gpui_kit::{
    ClickEvent, Context, Entity, EventEmitter, FontWeight, ImageSource, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _,
    StyledImage as _, Window, div, img, prelude::FluentBuilder as _, px, rgb, rgba,
};

use oxicord_model::collectible::{FrameCatalog, FramePreset};

use crate::{
    icons::icon,
    media::{Media, avatar},
    profile::modal,
    session::Session,
    theme,
};

/// Emitted when the dialog closes, carrying the frame that is now applied
/// (its sku, or nothing for None).
pub struct CloseFrameDialog(pub Option<String>);

impl EventEmitter<CloseFrameDialog> for FrameDialog {}

pub struct FrameDialog {
    session: Entity<Session>,
    media: Entity<Media>,
    catalog: FrameCatalog,
    draft: Option<String>,
    applied: Option<String>,
}

impl FrameDialog {
    pub fn new(session: Entity<Session>, media: Entity<Media>) -> Self {
        Self { session, media, catalog: FrameCatalog::bundled(), draft: None, applied: None }
    }

    pub fn opened(&mut self, applied: Option<String>, cx: &mut Context<Self>) {
        self.draft = applied.clone();
        self.applied = applied;
        cx.notify();
    }

    /// Owned frames: the catalog's list plus whatever this account is
    /// already wearing, which the live client also lists first.
    fn owned(&self) -> Vec<FramePreset> {
        let mut owned = self.catalog.owned.clone();
        if let Some(sku) = &self.applied
            && !owned.iter().any(|f| &f.sku_id == sku)
        {
            owned.push(match self.catalog.find(sku) {
                Some(found) => found.clone(),
                None => FramePreset {
                    sku_id: sku.clone(),
                    name: "Current frame".to_owned(),
                    locked: false,
                    pad_top: 14.0156,
                    pad_x: 2.57812,
                    pad_bottom: 9.76562,
                    top: None,
                    bottom: None,
                },
            });
        }
        owned
    }

    fn grid(&self, items: Vec<gpui_kit::AnyElement>) -> gpui_kit::Div {
        div().w(px(274.)).flex().flex_wrap().gap(px(12.)).children(items)
    }

    /// One 83.33 by 80 tile: the optional artwork over its optional 12 px
    /// label, with the optional round badge hanging off the top right.
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
            .w(px(83.33))
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
                    .p(px(2.))
                    .bg(rgba(0x0000_0099))
                    .child(icon(glyph, size, theme::TEXT_BRIGHT)),
            );
        }
        if selected {
            item = item.child(
                div()
                    .absolute()
                    .inset(px(-2.))
                    .rounded(px(6.))
                    .border(px(2.))
                    .border_color(rgb(theme::ACCENT)),
            );
        }
        item.into_any_element()
    }

    /// One frame tile: a preset portrait inside the frame's overflow padding,
    /// with the frame pictures drawn over it.
    fn frame_tile(
        &self,
        item: &FramePreset,
        selected: bool,
        badge: Option<(&'static str, f32)>,
        cx: &mut Context<Self>,
    ) -> gpui_kit::AnyElement {
        let sku = item.sku_id.clone();
        let sample = self.media.update(cx, |media, _| media.get(&self.catalog.sample));
        let top = item
            .top
            .as_ref()
            .and_then(|art| self.media.update(cx, |media, _| media.get(&art.url)));
        let bottom = item
            .bottom
            .as_ref()
            .and_then(|art| self.media.update(cx, |media, _| media.get(&art.url)));

        // The live tile preview is 55.33 by 68 with the frame's overflow as
        // padding; the layers scale with the inner portrait's width.
        let inner_w = 55.33 - 2.0 * item.pad_x;
        let inner_h = 68.0 - item.pad_top - item.pad_bottom;
        let ratio = item.pad_x / 55.33;
        let layer_left = -(inner_w * ratio);
        let layer_w = inner_w - 2.0 * layer_left;
        let top_top = -(inner_w * item.pad_top / 55.33);
        let bottom_top = inner_h - inner_w * item.pad_bottom / 55.33;

        let mut art = div().relative().w(px(inner_w)).h(px(inner_h));
        if let Some(image) = sample {
            // The live sample is a tall portrait drawn top aligned and cropped
            // to the inner card, not a centred cover.
            art = art.child(
                div()
                    .absolute()
                    .inset_0()
                    .overflow_hidden()
                    .rounded(px(4.))
                    .child(
                        img(ImageSource::Image(image))
                            .absolute()
                            .top_0()
                            .left_0()
                            .w_full()
                            .h(px(inner_w * 1.95))
                            .object_fit(gpui_kit::ObjectFit::Fill),
                    ),
            );
        }
        if let (Some(image), Some(layer)) = (top, &item.top) {
            let height = layer_w * layer.height as f32 / layer.width as f32;
            art = art.child(
                img(ImageSource::Image(image))
                    .absolute()
                    .left(px(layer_left))
                    .top(px(top_top))
                    .w(px(layer_w))
                    .h(px(height))
                    .object_fit(gpui_kit::ObjectFit::Fill),
            );
        }
        if let (Some(image), Some(layer)) = (bottom, &item.bottom) {
            let height = layer_w * layer.height as f32 / layer.width as f32;
            art = art.child(
                img(ImageSource::Image(image))
                    .absolute()
                    .left(px(layer_left))
                    .top(px(bottom_top))
                    .w(px(layer_w))
                    .h(px(height))
                    .object_fit(gpui_kit::ObjectFit::Fill),
            );
        }

        let mut tile = div()
            .id(gpui_kit::ElementId::Name(format!("frame-{sku}").into()))
            .w(px(83.33))
            .h(px(80.))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .px(px(14.))
            .py(px(6.))
            .rounded(px(4.))
            .bg(rgb(theme::BG_RAIL))
            .cursor_pointer()
            .child(
                div()
                    .w(px(55.33))
                    .h(px(68.))
                    .flex_none()
                    .overflow_hidden()
                    .pt(px(item.pad_top))
                    .pb(px(item.pad_bottom))
                    .px(px(item.pad_x))
                    .child(art),
            );
        if !item.locked {
            tile = tile.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.draft = Some(sku.clone());
                cx.notify();
            }));
        }
        if let Some((glyph, size)) = badge {
            tile = tile.child(
                div()
                    .absolute()
                    .top(px(-6.))
                    .right(px(-4.))
                    .size(px(20.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_full()
                    .p(px(2.))
                    .bg(rgba(0x0000_0099))
                    .child(icon(glyph, size, theme::TEXT_BRIGHT)),
            );
        }
        if selected {
            tile = tile.child(
                div()
                    .absolute()
                    .inset(px(-2.))
                    .rounded(px(6.))
                    .border(px(2.))
                    .border_color(rgb(theme::ACCENT)),
            );
        }
        tile.into_any_element()
    }

    /// The 300 wide profile card the live dialog shows to the right.
    fn preview_card(&self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let (me, accent, banner, pronouns, bio, tag, badges) = {
            let session = self.session.read(cx);
            let Some(me) = session.store.me().cloned() else {
                return div().into_any_element();
            };
            let details = session.account.details.ready().cloned();
            let profile = session.profiles.get(&me.id).cloned();
            let accent = details
                .as_ref()
                .and_then(|d| d.accent_color)
                .or(profile.as_ref().and_then(|p| p.accent_color))
                .unwrap_or(theme::ACCENT);
            let banner = details
                .as_ref()
                .and_then(|d| d.banner.clone())
                .map(|h| oxicord_media::cdn::user_banner(me.id, &h, 600))
                .or_else(|| {
                    profile
                        .as_ref()
                        .and_then(|p| p.banner.clone())
                        .map(|h| oxicord_media::cdn::user_banner(me.id, &h, 600))
                });
            let pronouns = profile.as_ref().map(|p| p.pronouns.clone()).unwrap_or_default();
            let bio = profile.as_ref().map(|p| p.bio.clone()).unwrap_or_default();
            let tag = profile.as_ref().and_then(|p| p.server_tag.as_ref()).map(|t| {
                (
                    t.tag.clone(),
                    t.badge
                        .as_ref()
                        .map(|hash| oxicord_media::cdn::clan_badge(t.guild_id, hash, 32)),
                )
            });
            let badges = profile
                .as_ref()
                .map(|p| {
                    p.badges
                        .iter()
                        .map(|b| (b.description.clone(), oxicord_media::cdn::badge_icon(&b.icon)))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            (me, accent, banner, pronouns, bio, tag, badges)
        };

        let name = me.display_name().to_owned();
        let avatar_url = oxicord_media::cdn::user_avatar(me.id, me.avatar.as_deref(), 128);
        let banner = match banner.as_deref().and_then(|url| self.media.update(cx, |m, _| m.get(url))) {
            Some(image) => div()
                .h(px(105.))
                .w_full()
                .flex_none()
                .overflow_hidden()
                .rounded_t(px(8.))
                .child(img(ImageSource::Image(image)).size_full().object_fit(gpui_kit::ObjectFit::Cover))
                .into_any_element(),
            None => div()
                .h(px(105.))
                .w_full()
                .flex_none()
                .rounded_t(px(8.))
                .bg(rgb(accent))
                .into_any_element(),
        };

        let mut who = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_x(px(6.))
            .gap_y(px(2.))
            .text_size(px(14.))
            .line_height(px(18.))
            .text_color(rgb(theme::TEXT_BRIGHT))
            .child(me.username.clone());
        if !pronouns.is_empty() {
            who = who
                .child(div().size(px(4.)).flex_none().rounded_full().bg(rgb(theme::TEXT)))
                .child(pronouns);
        }
        if let Some((text, badge)) = tag {
            let mut chip = div()
                .h(px(24.))
                .flex_none()
                .px(px(8.))
                .flex()
                .items_center()
                .gap(px(4.))
                .rounded(px(8.))
                .bg(rgba(0x94949c1f))
                .border_1()
                .border_color(rgba(0x9595a229))
                .text_size(px(12.))
                .line_height(px(16.))
                .text_color(rgb(theme::TEXT_BRIGHT));
            if let Some(url) = badge.as_deref().and_then(|url| self.media.update(cx, |m, _| m.get(url))) {
                chip = chip.child(
                    div()
                        .size(px(14.))
                        .flex_none()
                        .overflow_hidden()
                        .child(img(ImageSource::Image(url)).size_full()),
                );
            }
            who = who.child(chip.child(text));
        }
        if !badges.is_empty() {
            let mut strip = div()
                .h(px(22.))
                .flex_none()
                .px(px(3.))
                .flex()
                .items_center()
                .gap(px(2.))
                .rounded(px(8.))
                .border_1()
                .border_color(rgba(0x9696a033));
            for (_, url) in badges.iter().take(6) {
                if let Some(image) = self.media.update(cx, |m, _| m.get(url)) {
                    strip = strip.child(
                        div()
                            .size(px(20.))
                            .flex_none()
                            .overflow_hidden()
                            .child(img(ImageSource::Image(image)).size_full()),
                    );
                }
            }
            who = who.child(strip);
        }

        let identity = div()
            .flex()
            .flex_col()
            .child(
                div()
                    .text_size(px(20.))
                    .line_height(px(24.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(theme::TEXT))
                    .truncate()
                    .child(name.clone()),
            )
            .child(who);

        let body = div()
            .w_full()
            .flex_none()
            .flex()
            .flex_col()
            .gap(px(12.))
            .pt(px(4.))
            .pr(px(8.))
            .pb(px(8.))
            .pl(px(16.))
            .child(identity)
            .children((!bio.is_empty()).then(|| {
                div()
                    .text_size(px(16.))
                    .line_height(px(22.))
                    .text_color(rgb(theme::TEXT_BRIGHT))
                    .child(bio)
            }));

        let footer = div().w_full().flex_none().px(px(16.)).pb(px(12.)).child(
            div()
                .h(px(32.))
                .w_full()
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(8.))
                .bg(rgb(theme::ACCENT))
                .text_size(px(16.))
                .text_color(rgb(theme::TEXT_BRIGHT))
                .child("Example Button"),
        );

        div()
            .w(px(300.))
            .flex_none()
            .flex()
            .flex_col()
            .gap(px(8.))
            .pb(px(4.))
            .rounded(px(8.))
            .bg(rgb(theme::BG_PANEL))
            .overflow_hidden()
            .child(
                div()
                    .relative()
                    .h(px(150.))
                    .w_full()
                    .flex_none()
                    .child(banner)
                    .child(
                        div()
                            .absolute()
                            .left(px(16.))
                            .top(px(61.))
                            .child(
                                div()
                                    .relative()
                                    .size(px(80.))
                                    .child(avatar(&self.media, &avatar_url, 80., &name, cx))
                                    .child(
                                        div()
                                            .absolute()
                                            .left(px(56.))
                                            .top(px(56.))
                                            .size(px(24.))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .rounded_full()
                                            .bg(rgb(theme::BG_PANEL))
                                            .child(
                                                div().size(px(16.)).rounded_full().bg(rgb(theme::TEXT_MUTED)),
                                            ),
                                    ),
                            ),
                    ),
            )
            .child(body)
            .child(footer)
            .into_any_element()
    }
}

impl Render for FrameDialog {
    fn render(&mut self, _: &mut gpui_kit::Window, cx: &mut Context<Self>) -> impl IntoElement {
        let owned = self.owned();
        let owned_skus: Vec<&str> = owned.iter().map(|f| f.sku_id.as_str()).collect();

        let none_selected = self.draft.is_none();
        let none = self.item(
            "frame-none",
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
            "frame-shop",
            div().mb(px(4.)).child(icon("store", 23., theme::TEXT_BRIGHT)).into_any_element(),
            "Shop",
            false,
            None,
            None,
        );
        let mut your_items = vec![none, shop];
        for item in &owned {
            your_items.push(self.frame_tile(
                item,
                self.draft.as_deref() == Some(item.sku_id.as_str()),
                None,
                cx,
            ));
        }

        let shop_items: Vec<_> = self
            .catalog
            .shop
            .iter()
            .filter(|f| !owned_skus.contains(&f.sku_id.as_str()))
            .map(|f| self.frame_tile(f, false, Some(("lock-badge", 12.)), cx))
            .collect();

        let section = |label: &'static str, grid: gpui_kit::Div| {
            div()
                .flex()
                .flex_col()
                .gap(px(8.))
                .pb(px(12.))
                .child(
                    div()
                        .text_size(px(16.))
                        .line_height(px(20.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(rgb(theme::TEXT_BRIGHT))
                        .child(label),
                )
                .child(grid)
        };

        let list = div()
            .id("frame-list")
            .ml(px(-12.))
            .mt(px(2.))
            .w(px(298.))
            .h(px(678.))
            .flex_none()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap(px(4.))
            .px(px(12.))
            .pb(px(4.))
            .child(section("Your Frames", self.grid(your_items)))
            .child(section("See What's in Shop", self.grid(shop_items)));

        let preview = div()
            .mt(px(6.5))
            .ml(px(14.))
            .w(px(310.))
            .flex_none()
            .flex()
            .flex_col()
            .gap(px(12.))
            .pl(px(10.))
            .child(self.preview_card(cx));

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
                    .id("frame-cancel")
                    .w(px(100.))
                    .h(px(40.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(8.))
                    .border_1()
                    .border_color(rgba(0x97979f29))
                    .bg(rgba(0x97979f1f))
                    .cursor_pointer()
                    .text_size(px(16.))
                    .text_color(rgb(theme::TEXT_BRIGHT))
                    .hover(|s| s.bg(rgba(0x97979f33)))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        cx.emit(CloseFrameDialog(this.applied.clone()));
                    }))
                    .child("Cancel"),
            )
            .child(
                div()
                    .id("frame-apply")
                    .w(px(100.))
                    .h(px(40.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(8.))
                    .border_1()
                    .border_color(rgba(0xffff_ff14))
                    .bg(rgb(theme::ACCENT))
                    .when(!dirty, |d| d.opacity(0.5))
                    .cursor_pointer()
                    .text_size(px(16.))
                    .text_color(rgb(theme::TEXT_BRIGHT))
                    .hover(|s| s.bg(rgb(theme::ACCENT_HOVER)))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        cx.emit(CloseFrameDialog(this.draft.clone()));
                    }))
                    .child("Apply"),
            );

        let dialog = div()
            .id("frame-dialog")
            .occlude()
            .w(px(663.))
            .h(px(802.))
            .flex_none()
            .flex()
            .flex_col()
            .overflow_hidden()
            .rounded(px(theme::RADIUS_MD))
            .bg(rgb(theme::BG_PANEL))
            .border_1()
            .border_color(rgba(0x97979f3d))
            .child(
                div()
                    .h(px(40.))
                    .flex_none()
                    .flex()
                    .items_start()
                    .justify_between()
                    .px(px(24.))
                    .pt(px(16.))
                    .child(
                        div()
                            .text_size(px(20.))
                            .line_height(px(24.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(theme::TEXT_BRIGHT))
                            .child("Change Profile Frame"),
                    )
                    .child(
                        div()
                            .id("frame-close")
                            .size(px(24.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(3.))
                            .opacity(0.5)
                            .cursor_pointer()
                            .hover(|s| s.opacity(1.))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                cx.emit(CloseFrameDialog(this.applied.clone()));
                            }))
                            .child(icon("x", 24., theme::TEXT_BRIGHT)),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .gap(px(2.))
                    .px(px(24.))
                    .pr(px(16.))
                    .pt(px(8.))
                    .child(list)
                    .child(preview),
            )
            .child(footer);

        modal(
            dialog.into_any_element(),
            Box::new(cx.listener(|this, _: &ClickEvent, _, cx| {
                cx.emit(CloseFrameDialog(this.applied.clone()));
            })),
        )
        .into_any_element()
    }
}

type ClickRun = Box<dyn Fn(&ClickEvent, &mut Window, &mut gpui_kit::App) + 'static>;
