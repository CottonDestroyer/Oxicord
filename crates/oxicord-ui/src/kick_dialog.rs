use gpui_kit::{
    ClickEvent, Context, Entity, EventEmitter, InteractiveElement as _, IntoElement, ParentElement as _,
    Render, StatefulInteractiveElement as _, Styled as _, Window, div, px, rgb,
};
use oxicord_model::Snowflake;

use crate::{backend::{Backend, Command}, profile::modal, session::Session, theme};

pub struct CloseKick;

impl EventEmitter<CloseKick> for KickDialog {}

pub struct KickDialog {
    backend: Backend,
    target: Option<(Snowflake, Snowflake, String)>,
}

impl KickDialog {
    pub fn new(_session: Entity<Session>, backend: Backend, _: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self { backend, target: None }
    }

    pub fn opened(&mut self, guild: Snowflake, user: Snowflake, name: String, _: &mut Window, cx: &mut Context<Self>) {
        self.target = Some((guild, user, name));
        cx.notify();
    }

    fn confirm(&mut self, cx: &mut Context<Self>) {
        let Some((guild, user, _)) = self.target else { return };
        self.backend.send(Command::KickMember { guild, user });
        cx.emit(CloseKick);
    }
}

impl Render for KickDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (name, has_target) = match &self.target {
            Some((_, _, name)) => (name.clone(), true),
            None => (String::new(), false),
        };

        let card = div()
            .id("kick-card")
            .occlude()
            .w(px(440.))
            .p(px(24.))
            .flex()
            .flex_col()
            .gap(px(16.))
            .rounded(px(theme::RADIUS))
            .border_1()
            .border_color(rgb(theme::BORDER_MUTED))
            .bg(rgb(theme::BG_PANEL))
            .child(
                div()
                    .text_size(px(18.))
                    .font_weight(gpui_kit::FontWeight::BOLD)
                    .text_color(rgb(theme::TEXT_BRIGHT))
                    .child("Kick member"),
            )
            .child(
                div()
                    .text_color(rgb(theme::TEXT))
                    .child(format!("Kick {name}? They will be removed from the server.")),
            )
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(8.))
                    .child(
                        div()
                            .id("kick-cancel")
                            .h(px(36.))
                            .px(px(16.))
                            .flex()
                            .items_center()
                            .rounded(px(theme::RADIUS))
                            .bg(rgb(theme::BG_HOVER))
                            .text_color(rgb(theme::TEXT_BRIGHT))
                            .cursor_pointer()
                            .hover(|s| s.opacity(0.85))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.target = None;
                                cx.emit(CloseKick);
                            }))
                            .child("Cancel"),
                    )
                    .child(
                        div()
                            .id("kick-confirm")
                            .h(px(36.))
                            .px(px(16.))
                            .flex()
                            .items_center()
                            .rounded(px(theme::RADIUS))
                            .bg(rgb(theme::DANGER))
                            .text_color(rgb(theme::TEXT_BRIGHT))
                            .cursor_pointer()
                            .hover(|s| s.opacity(0.85))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.confirm(cx)))
                            .child("Kick"),
                    ),
            );

        if has_target {
            modal(card.into_any_element(), Box::new(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(CloseKick))))
        } else {
            div().into_any_element()
        }
    }
}