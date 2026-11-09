//! The Change Nameplate dialog.

use gpui_kit::{
    ClickEvent, Context, Entity, EventEmitter, FontWeight, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _,
    prelude::FluentBuilder as _, px, rgb, div,
};

use oxicord_model::collectible::NameplateCatalog;

use crate::{
    edit_profile_modal::nameplate_row,
    icons::icon,
    media::{Media, avatar},
    profile::modal,
    session::Session,
    theme,
};

/// Emitted when the dialog closes, carrying the nameplate that is now applied
/// (its sku, or nothing for None).
pub struct CloseNameplateDialog(pub Option<String>);

impl EventEmitter<CloseNameplateDialog> for NameplateDialog {}

pub struct NameplateDialog {
    session: Entity<Session>,
    media: Entity<Media>,
    catalog: NameplateCatalog,
    draft: Option<String>,
    applied: Option<String>,
}

impl NameplateDialog {
    pub fn new(session: Entity<Session>, media: Entity<Media>) -> Self {
        Self { session, media, catalog: NameplateCatalog::bundled(), draft: None, applied: None }
    }

    pub fn opened(&mut self, applied: Option<String>, cx: &mut Context<Self>) {
        self.draft = applied.clone();
        self.applied = applied;
        cx.notify();
    }
}

impl Render for NameplateDialog {
    fn render(&mut self, _: &mut gpui_kit::Window, cx: &mut Context<Self>) -> impl IntoElement {
        let me = self.session.read(cx).store.me().cloned();
        let avatar_url = me
            .as_ref()
            .map(|m| oxicord_media::cdn::user_avatar(m.id, m.avatar.as_deref(), 64))
            .unwrap_or_default();
        let me_name = me.as_ref().map(|m| m.display_name().to_owned()).unwrap_or_default();

        let none_selected = self.draft.is_none();
        let none = self.control(
            "None",
            true,
            none_selected,
            Box::new(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.draft = None;
                cx.notify();
            })),
        );
        let shop = self.control("Shop", false, false, Box::new(cx.listener(|_, _: &ClickEvent, _, _| {})));

        let mut owned = div().flex().flex_col().gap(px(12.)).w(px(224.));
        for item in &self.catalog.owned {
            let selected = self.draft.as_deref() == Some(item.sku_id.as_str());
            let sku = item.sku_id.clone();
            owned = owned.child(
                div()
                    .id(gpui_kit::ElementId::Name(format!("owned-{}", item.sku_id).into()))
                    .rounded(px(8.))
                    .cursor_pointer()
                    .when(selected, |d| d.border(px(2.)).border_color(rgb(theme::ACCENT)))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.draft = Some(sku.clone());
                        cx.notify();
                    }))
                    .child(nameplate_row(item, &avatar_url, &self.media, 56., cx)),
            );
        }

        let mut shop_list = div().flex().flex_col().gap(px(12.)).w(px(224.));
        for item in &self.catalog.shop {
            shop_list = shop_list.child(
                div()
                    .rounded(px(8.))
                    .opacity(if item.locked { 0.7 } else { 1. })
                    .child(nameplate_row(item, &avatar_url, &self.media, 56., cx)),
            );
        }

        let inventory = div()
            .w(px(232.))
            .flex_none()
            .flex()
            .flex_col()
            .gap(px(16.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .child(
                        div()
                            .text_size(px(12.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(theme::TEXT_BRIGHT))
                            .child("Your Nameplates"),
                    )
                    .child(div().flex().gap(px(12.)).child(none).child(shop))
                    .child(owned),
            )
            .child(
                div()
                    .text_size(px(14.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(theme::TEXT_BRIGHT))
                    .child("See What's in Shop"),
            )
            .child(shop_list);

        let mut rows = div().flex().flex_col().gap(px(12.)).w(px(224.));
        rows = rows.child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(avatar(&self.media, &avatar_url, 28., &me_name, cx))
                .child(
                    div()
                        .text_size(px(14.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(rgb(theme::TEXT_BRIGHT))
                        .child(me_name.clone()),
                ),
        );
        for _ in 0..4 {
            rows = rows.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(div().size(px(28.)).flex_none().rounded_full().bg(rgb(theme::BG_ACTIVE)))
                    .child(div().flex_1().h(px(12.)).rounded_full().bg(rgb(theme::BG_ACTIVE))),
            );
        }

        let preview = div().w(px(276.)).flex_none().flex().flex_col().child(
            div()
                .w_full()
                .h(px(190.))
                .flex()
                .items_center()
                .justify_center()
                .overflow_hidden()
                .rounded(px(8.))
                .bg(rgb(theme::BG_CARD))
                .child(rows),
        );

        let dirty = self.draft != self.applied;
        let footer = div()
            .flex()
            .items_center()
            .justify_end()
            .gap(px(8.))
            .px(px(16.))
            .h(px(56.))
            .child(
                div()
                    .id("nameplate-cancel")
                    .h(px(32.))
                    .px(px(16.))
                    .flex()
                    .items_center()
                    .rounded(px(4.))
                    .bg(rgb(theme::BG_ACTIVE))
                    .cursor_pointer()
                    .text_size(px(14.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(theme::TEXT_BRIGHT))
                    .hover(|s| s.bg(rgb(theme::BG_HOVER)))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        cx.emit(CloseNameplateDialog(this.applied.clone()));
                    }))
                    .child("Cancel"),
            )
            .child(
                div()
                    .id("nameplate-apply")
                    .h(px(32.))
                    .px(px(16.))
                    .flex()
                    .items_center()
                    .rounded(px(4.))
                    .bg(rgb(theme::ACCENT))
                    .when(!dirty, |d| d.opacity(0.5))
                    .cursor_pointer()
                    .text_size(px(14.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(theme::TEXT_BRIGHT))
                    .hover(|s| s.bg(rgb(theme::SUCCESS_HOVER)))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        cx.emit(CloseNameplateDialog(this.draft.clone()));
                    }))
                    .child("Apply"),
            );

        let dialog = div()
            .id("nameplate-dialog")
            .occlude()
            .w(px(598.))
            .h(px(501.))
            .flex_none()
            .flex()
            .flex_col()
            .overflow_hidden()
            .rounded(px(theme::RADIUS))
            .bg(rgb(theme::BG_MENU))
            .border_1()
            .border_color(rgb(theme::BORDER))
            .child(
                div()
                    .h(px(40.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_between()
                    .pl(px(16.))
                    .pr(px(8.))
                    .child(
                        div()
                            .text_size(px(16.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(theme::TEXT_BRIGHT))
                            .child("Change Nameplate"),
                    )
                    .child(
                        div()
                            .id("nameplate-close")
                            .size(px(32.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(theme::RADIUS))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(theme::BG_HOVER)))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                cx.emit(CloseNameplateDialog(this.applied.clone()));
                            }))
                            .child(icon("x", 20., theme::TEXT_MUTED)),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .gap(px(16.))
                    .px(px(16.))
                    .child(inventory)
                    .child(preview),
            )
            .child(footer);

        modal(
            dialog.into_any_element(),
            Box::new(cx.listener(|this, _: &ClickEvent, _, cx| {
                cx.emit(CloseNameplateDialog(this.applied.clone()));
            })),
        )
        .into_any_element()
    }
}

type ClickRun = Box<dyn Fn(&ClickEvent, &mut gpui_kit::Window, &mut gpui_kit::App) + 'static>;

impl NameplateDialog {
    fn control(
        &self,
        label: &'static str,
        with_icon: bool,
        selected: bool,
        on_click: ClickRun,
    ) -> gpui_kit::AnyElement {
        let mut button = div()
            .id(label)
            .w(px(106.))
            .h(px(80.))
            .flex_none()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(4.))
            .rounded(px(4.))
            .bg(rgb(theme::BG_INPUT))
            .border(px(2.))
            .border_color(rgb(if selected { theme::ACCENT } else { theme::BG_INPUT }))
            .cursor_pointer()
            .text_size(px(12.))
            .text_color(rgb(theme::TEXT_BRIGHT))
            .on_click(on_click);
        if with_icon {
            button = button.child(icon("ban", 22., theme::TEXT_BRIGHT));
        }
        button.child(label).into_any_element()
    }
}

