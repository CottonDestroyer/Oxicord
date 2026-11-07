//! The "Time Out" dialog: Discord asks how long to mute a member for.

use gpui_kit::{
    ClickEvent, Context, Entity, EventEmitter, FontWeight, InteractiveElement as _, IntoElement, KeyDownEvent,
    ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _, Window, div, px, rgb,
};
use oxicord_model::Snowflake;

use crate::{backend::{Backend, Command}, profile::modal, theme};

/// Discord's timeout presets, in seconds.
const DURATIONS: &[( &str, i64)] = &[
    ("60 seconds", 60),
    ("5 minutes", 300),
    ("10 minutes", 600),
    ("1 hour", 3600),
    ("1 day", 86_400),
    ("1 week", 604_800),
    ("30 days", 2_592_000),
];

pub struct CloseTimeout;

impl EventEmitter<CloseTimeout> for TimeoutDialog {}

pub struct TimeoutDialog {
    backend: Backend,
    target: Option<(Snowflake, Snowflake, String)>,
    selected: usize,
}

impl TimeoutDialog {
    pub fn new(_session: Entity<crate::session::Session>, backend: Backend, _: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self { backend, target: None, selected: 3 }
    }

    /// Opens for `user` in `guild`, naming them in the warning.
    pub fn opened(&mut self, guild: Snowflake, user: Snowflake, name: String, _: &mut Window, cx: &mut Context<Self>) {
        self.target = Some((guild, user, name));
        self.selected = 3;
        cx.notify();
    }

    fn confirm(&mut self, cx: &mut Context<Self>) {
        let Some((guild, user, _)) = self.target else { return };
        let seconds = DURATIONS[self.selected].1;
        let until = (chrono::Utc::now() + chrono::Duration::seconds(seconds))
            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        self.backend.send(Command::TimeoutMember { guild, user, until: Some(until) });
        cx.emit(CloseTimeout);
    }

    fn remove(&mut self, cx: &mut Context<Self>) {
        let Some((guild, user, _)) = self.target else { return };
        self.backend.send(Command::TimeoutMember { guild, user, until: None });
        cx.emit(CloseTimeout);
    }
}

impl Render for TimeoutDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (name, has_target) = match &self.target {
            Some((_, _, name)) => (name.clone(), true),
            None => (String::new(), false),
        };
        let selected = self.selected;

        let options = div()
            .id("timeout-options")
            .flex()
            .flex_col()
            .gap(px(4.))
            .children(DURATIONS.iter().enumerate().map(|(i, (label, _))| {
                let is_selected = i == selected;
                div()
                    .id(("timeout-opt", i))
                    .flex()
                    .items_center()
                    .justify_between()
                    .h(px(36.))
                    .px(px(12.))
                    .rounded(px(theme::RADIUS))
                    .cursor_pointer()
                    .bg(if is_selected { rgb(theme::BG_HOVER) } else { rgb(theme::BG_PANEL) })
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.selected = i;
                        cx.notify();
                    }))
                    .child(div().text_size(px(14.)).text_color(rgb(theme::TEXT)).child(*label))
                    .child(if is_selected {
                        div().text_size(px(14.)).text_color(rgb(theme::ACCENT)).child("✓")
                    } else {
                        div()
                    })
            }));

        let card = div()
            .id("timeout-card")
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
                    cx.emit(CloseTimeout);
                    cx.stop_propagation();
                }
            }))
            .child(
                div()
                    .text_size(px(20.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(theme::HEADER_PRIMARY))
                    .child(format!("Time Out {name}?")),
            )
            .child(
                div()
                    .text_size(px(14.))
                    .text_color(rgb(theme::TEXT_MUTED))
                    .child("They won't be able to send messages, react, or join voice for the duration you choose."),
            )
            .child(options)
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(8.))
                    .child(
                        div()
                            .id("timeout-remove")
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
                            .child("Remove Timeout")
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.remove(cx))),
                    )
                    .child(
                        div()
                            .id("timeout-cancel")
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
                            .on_click(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(CloseTimeout))),
                    )
                    .child(
                        div()
                            .id("timeout-confirm")
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
                            .child("Time Out")
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.confirm(cx))),
                    ),
            );

        if has_target {
            modal(card.into_any_element(), Box::new(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(CloseTimeout))))
        } else {
            div().into_any_element()
        }
    }
}
