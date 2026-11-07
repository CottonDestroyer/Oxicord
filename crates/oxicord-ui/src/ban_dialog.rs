//! The "Ban Member" confirmation: Discord asks before banning and offers to
//! delete the member's recent messages.

use gpui_kit::{
    ClickEvent, Context, Entity, EventEmitter, FontWeight, InteractiveElement as _, IntoElement, KeyDownEvent,
    ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _, Window, div, px, rgb,
};
use oxicord_model::Snowflake;

use crate::{backend::{Backend, Command}, profile::modal, session::Session, theme};

pub struct CloseBan;

impl EventEmitter<CloseBan> for BanDialog {}

pub struct BanDialog {
    backend: Backend,
    target: Option<(Snowflake, Snowflake, String)>,
    delete_history: bool,
}

impl BanDialog {
    pub fn new(_session: Entity<Session>, backend: Backend, _: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self { backend, target: None, delete_history: false }
    }

    /// Opens for `user` in `guild`, naming them in the warning.
    pub fn opened(&mut self, guild: Snowflake, user: Snowflake, name: String, _: &mut Window, cx: &mut Context<Self>) {
        self.target = Some((guild, user, name));
        self.delete_history = false;
        cx.notify();
    }

    fn confirm(&mut self, cx: &mut Context<Self>) {
        let Some((guild, user, _)) = self.target else { return };
        let days = if self.delete_history { 7 } else { 0 };
        self.backend.send(Command::BanMember { guild, user, delete_message_days: days });
        cx.emit(CloseBan);
    }
}

impl Render for BanDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (name, has_target) = match &self.target {
            Some((_, _, name)) => (name.clone(), true),
            None => (String::new(), false),
        };
        let delete_history = self.delete_history;

        let card = div()
            .id("ban-card")
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
            .capture_key_down(cx.listener(|_, event: &KeyDownEvent, _, cx| {
                if event.keystroke.key == "escape" {
                    cx.emit(CloseBan);
                    cx.stop_propagation();
                }
            }))
            .child(
                div()
                    .text_size(px(20.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(theme::HEADER_PRIMARY))
                    .child(format!("Ban {name}?")),
            )
            .child(
                div()
                    .text_size(px(14.))
                    .text_color(rgb(theme::TEXT_MUTED))
                    .child(
                        "They won't be able to rejoin this server, and you'll need to manually unban them later.",
                    ),
            )
            .child(
                div()
                    .id("ban-delete-history")
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.delete_history = !this.delete_history;
                        cx.notify();
                    }))
                    .child(
                        div()
                            .size(px(18.))
                            .rounded(px(4.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .border_1()
                            .border_color(rgb(theme::BORDER))
                            .bg(if delete_history { rgb(theme::ACCENT) } else { rgb(theme::BG_HOVER) })
                            .child(if delete_history { "✓" } else { "" }),
                    )
                    .child(
                        div()
                            .text_size(px(14.))
                            .text_color(rgb(theme::TEXT))
                            .child("Delete this member's messages for the last 7 days"),
                    ),
            )
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(8.))
                    .child(
                        div()
                            .id("ban-cancel")
                            .h(px(40.))
                            .px(px(20.))
                            .flex()
                            .items_center()
                            .rounded(px(theme::RADIUS))
                            .cursor_pointer()
                            .text_size(px(14.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgb(theme::TEXT_BRIGHT))
                            .bg(rgb(theme::BG_HOVER))
                            .hover(|s| s.opacity(0.85))
                            .child("Cancel")
                            .on_click(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(CloseBan))),
                    )
                    .child(
                        div()
                            .id("ban-confirm")
                            .h(px(40.))
                            .px(px(20.))
                            .flex()
                            .items_center()
                            .rounded(px(theme::RADIUS))
                            .cursor_pointer()
                            .text_size(px(14.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgb(0xffffff))
                            .bg(rgb(theme::DANGER))
                            .hover(|s| s.opacity(0.85))
                            .child("Ban")
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.confirm(cx))),
                    ),
            );

        if has_target {
            modal(card.into_any_element(), Box::new(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(CloseBan))))
        } else {
            div().into_any_element()
        }
    }
}
