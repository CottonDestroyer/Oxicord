//! The "Change Friend Nickname" dialog: the personal name we keep for a friend,
//! shown only in our own DMs with them.

use gpui_kit::{
    ClickEvent, Context, Entity, EventEmitter, FontWeight, InteractiveElement as _, IntoElement, KeyDownEvent,
    ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _, Subscription, Window, div, px, rgb,
    component::input::{Input, InputEvent, InputState},
};
use gpui_kit::AppContext as _;
use oxicord_model::Snowflake;

use crate::{
    backend::{Backend, Command},
    profile::modal,
    session::Session,
    theme,
};

/// Discord's limit for a friend nickname.
const MAX_CHARS: usize = 32;

pub struct CloseFriendNick;

impl EventEmitter<CloseFriendNick> for FriendNickDialog {}

pub struct FriendNickDialog {
    session: Entity<Session>,
    backend: Backend,
    input: Entity<InputState>,
    user: Option<Snowflake>,
    _subscriptions: Vec<Subscription>,
}

impl FriendNickDialog {
    pub fn new(session: Entity<Session>, backend: Backend, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Nickname"));
        let subscriptions = vec![
            cx.observe(&session, |_, _, cx| cx.notify()),
            cx.subscribe(&input, |this, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::PressEnter { .. }) {
                    this.save(cx);
                }
            }),
        ];
        Self { session, backend, input, user: None, _subscriptions: subscriptions }
    }

    /// Opens for `user`, prefilling the box with the nickname we already set.
    pub fn opened(&mut self, user: Snowflake, window: &mut Window, cx: &mut Context<Self>) {
        self.user = Some(user);
        let current = self.session.read(cx).store.relationship_nick(user).unwrap_or_default();
        self.input.update(cx, |input, cx| {
            input.set_value(current, window, cx);
            input.focus(window, cx);
        });
        cx.notify();
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        let Some(user) = self.user else { return };
        let nick: String = self.input.read(cx).value().trim().chars().take(MAX_CHARS).collect();
        self.send(user, (!nick.is_empty()).then_some(nick), cx);
    }

    fn send(&mut self, user: Snowflake, nick: Option<String>, cx: &mut Context<Self>) {
        // Apply the change locally right away so the friends list and DMs update
        // immediately, then persist it to Discord.
        self.session.update(cx, |session, cx| {
            session.store.set_relationship_nick(user, nick.clone());
            cx.notify();
        });
        self.backend.send(Command::SetFriendNick { user, nick });
        cx.emit(CloseFriendNick);
    }
}

impl Render for FriendNickDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let button = |id: &'static str, label: &'static str, primary: bool| {
            div()
                .id(id)
                .h(px(40.))
                .px(px(20.))
                .flex()
                .items_center()
                .rounded(px(theme::RADIUS))
                .cursor_pointer()
                .text_size(px(14.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(rgb(if primary { 0xffffff } else { theme::TEXT_BRIGHT }))
                .bg(rgb(if primary { theme::ACCENT } else { theme::BG_HOVER }))
                .hover(|s| s.opacity(0.85))
                .child(label)
        };

        let card = div()
            .id("friend-nick-card")
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
                    cx.emit(CloseFriendNick);
                    cx.stop_propagation();
                }
            }))
            .child(
                div()
                    .text_size(px(20.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(theme::HEADER_PRIMARY))
                    .child("Change Friend Nickname"),
            )
            .child(
                div()
                    .text_size(px(14.))
                    .text_color(rgb(theme::TEXT_MUTED))
                    .child(
                        "Find a friend faster with a personal nickname. It will only be visible to you in your direct messages.",
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(div().text_size(px(12.)).text_color(rgb(theme::TEXT_MUTED)).child("Friend Nickname"))
                    .child(Input::new(&self.input)),
            )
            .child(
                div()
                    .flex()
                    .justify_between()
                    .child(
                        button("friend-nick-reset", "Reset Friend Nickname", false)
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                if let Some(user) = this.user {
                                    this.send(user, None, cx);
                                }
                            })),
                    )
                    .child(
                        button("friend-nick-save", "Save", true)
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.save(cx))),
                    ),
            );

        modal(card.into_any_element(), Box::new(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(CloseFriendNick))))
    }
}
