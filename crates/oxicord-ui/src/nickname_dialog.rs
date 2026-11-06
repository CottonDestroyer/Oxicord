//! The "Change Nickname" dialog: the name others see for you in one server.

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

/// Discord's limit for a server nickname.
const MAX_CHARS: usize = 32;

pub struct CloseNickname;

impl EventEmitter<CloseNickname> for NicknameDialog {}

pub struct NicknameDialog {
    session: Entity<Session>,
    backend: Backend,
    input: Entity<InputState>,
    guild: Option<Snowflake>,
    _subscriptions: Vec<Subscription>,
}

impl NicknameDialog {
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
        Self { session, backend, input, guild: None, _subscriptions: subscriptions }
    }

    /// Opens on `guild`, asking the server what our nickname there is now.
    pub fn opened(&mut self, guild: Snowflake, window: &mut Window, cx: &mut Context<Self>) {
        self.guild = Some(guild);
        let current = self.session.read(cx).own_nicks.get(&guild).cloned().flatten().unwrap_or_default();
        self.input.update(cx, |input, cx| {
            input.set_value(current, window, cx);
            input.focus(window, cx);
        });
        self.backend.send(Command::LoadOwnNick(guild));
        cx.notify();
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        let Some(guild) = self.guild else { return };
        let nick: String = self.input.read(cx).value().trim().chars().take(MAX_CHARS).collect();
        self.send(guild, (!nick.is_empty()).then_some(nick), cx);
    }

    fn send(&mut self, guild: Snowflake, nick: Option<String>, cx: &mut Context<Self>) {
        self.session.update(cx, |s, _| {
            s.own_nicks.insert(guild, nick.clone());
        });
        self.backend.send(Command::SetNickname { guild, nick });
        cx.emit(CloseNickname);
    }
}

impl Render for NicknameDialog {
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
            .id("nickname-card")
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
                    cx.emit(CloseNickname);
                    cx.stop_propagation();
                }
            }))
            .child(
                div()
                    .text_size(px(20.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(theme::HEADER_PRIMARY))
                    .child("Change Nickname"),
            )
            .child(
                div()
                    .text_size(px(14.))
                    .text_color(rgb(theme::TEXT_MUTED))
                    .child("Pick a name just for this server. Leave it empty to use your usual name."),
            )
            .child(Input::new(&self.input))
            .child(
                div()
                    .flex()
                    .justify_between()
                    .child(button("nickname-reset", "Reset Nickname", false).on_click(cx.listener(
                        |this, _: &ClickEvent, _, cx| {
                            if let Some(guild) = this.guild {
                                this.send(guild, None, cx);
                            }
                        },
                    )))
                    .child(
                        button("nickname-save", "Save", true)
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.save(cx))),
                    ),
            );

        modal(card.into_any_element(), Box::new(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(CloseNickname))))
    }
}
