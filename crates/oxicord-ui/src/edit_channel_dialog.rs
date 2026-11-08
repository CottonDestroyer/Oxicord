//! The "Edit Channel" dialog reached from a channel's hover button: rename the
//! channel and set its topic. The change goes through the same admin path the
//! server settings use, and the gateway reports it back to the store.

use gpui_kit::{
    ClickEvent, Context, Entity, EventEmitter, FontWeight, InteractiveElement as _, IntoElement, KeyDownEvent,
    ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _, Subscription, Window, div, px, rgb,
    component::input::{Input, InputEvent, InputState},
};
use gpui_kit::AppContext as _;
use serde_json::json;
use oxicord_model::Snowflake;

use crate::{
    admin::AdminRequest,
    backend::{Backend, Command},
    profile::modal,
    session::Session,
    theme,
};

pub struct CloseEditChannel;

impl EventEmitter<CloseEditChannel> for EditChannelDialog {}

pub struct EditChannelDialog {
    session: Entity<Session>,
    backend: Backend,
    channel: Option<Snowflake>,
    name: Entity<InputState>,
    topic: Entity<InputState>,
    follow_target: Entity<InputState>,
    _subscriptions: Vec<Subscription>,
}

impl EditChannelDialog {
    pub fn new(session: Entity<Session>, backend: Backend, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let name = cx.new(|cx| InputState::new(window, cx).placeholder("Channel name"));
        let topic = cx.new(|cx| InputState::new(window, cx).placeholder("Add a topic"));
        let follow_target = cx.new(|cx| InputState::new(window, cx).placeholder("Target channel ID"));
        let subscriptions = vec![
            cx.observe(&session, |_, _, cx| cx.notify()),
            cx.subscribe(&name, |this, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::PressEnter { .. }) {
                    this.save(cx);
                }
            }),
            cx.subscribe(&topic, |this, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::PressEnter { .. }) {
                    this.save(cx);
                }
            }),
        ];
        Self { session, backend, channel: None, name, topic, follow_target, _subscriptions: subscriptions }
    }

    /// Opens for `channel`, prefilling the name and topic the store has.
    pub fn opened(&mut self, channel: Snowflake, window: &mut Window, cx: &mut Context<Self>) {
        self.channel = Some(channel);
        let (name, topic) = {
            let store = &self.session.read(cx).store;
            match store.channel(channel) {
                Some(ch) => (ch.name.clone().unwrap_or_default(), ch.topic.clone().unwrap_or_default()),
                None => (String::new(), String::new()),
            }
        };
        self.name.update(cx, |input, cx| {
            input.set_value(name, window, cx);
            input.focus(window, cx);
        });
        self.topic.update(cx, |input, cx| {
            input.set_value(topic, window, cx);
        });
        self.follow_target.update(cx, |input, cx| {
            input.set_value(String::new(), window, cx);
        });
        cx.notify();
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        let Some(channel) = self.channel else { return };
        let name: String = self.name.read(cx).value().trim().to_owned();
        let topic: String = self.topic.read(cx).value().trim().to_owned();
        if name.is_empty() {
            // Discord requires a name; keep the current one rather than clearing.
            return;
        }
        let fields = json!({
            "name": name,
            "topic": if topic.is_empty() { serde_json::Value::Null } else { json!(topic) },
        });
        self.session.update(cx, |session, cx| {
            session.request_admin(&self.backend, AdminRequest::EditChannel { channel, fields }, cx);
        });
        cx.emit(CloseEditChannel);
    }
}

impl Render for EditChannelDialog {
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
            .id("edit-channel-card")
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
                    cx.emit(CloseEditChannel);
                    cx.stop_propagation();
                }
            }))
            .child(
                div()
                    .text_size(px(20.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(theme::HEADER_PRIMARY))
                    .child("Edit Channel"),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(div().text_size(px(12.)).text_color(rgb(theme::TEXT_MUTED)).child("CHANNEL NAME"))
                    .child(Input::new(&self.name)),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(div().text_size(px(12.)).text_color(rgb(theme::TEXT_MUTED)).child("CHANNEL TOPIC"))
                    .child(Input::new(&self.topic)),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(div().text_size(px(12.)).text_color(rgb(theme::TEXT_MUTED)).child("FOLLOW TARGET CHANNEL ID"))
                    .child(Input::new(&self.follow_target))
                    .child(button("follow-channel", "Follow", true).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        let Some(channel) = this.channel else { return };
                        let text = this.follow_target.read(cx).value().trim().to_owned();
                        if let Ok(target) = text.parse::<u64>() {
                            this.backend.send(Command::FollowChannel { source: channel, target: oxicord_model::Snowflake(target) });
                        }
                    }))),
            )
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(8.))
                    .child(
                        button("edit-channel-cancel", "Cancel", false)
                            .on_click(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(CloseEditChannel))),
                    )
                    .child(
                        button("edit-channel-save", "Save", true)
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.save(cx))),
                    ),
            );

        modal(card.into_any_element(), Box::new(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(CloseEditChannel))))
    }
}
