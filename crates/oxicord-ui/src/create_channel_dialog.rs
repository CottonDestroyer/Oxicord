//! The "Create Channel" dialog reached from a category header: choose a channel
//! type (Text / Voice / Forum), name it, and post the request. Reuses the same
//! admin path the server settings use; the gateway reports the new channel back.

use gpui_kit::{
    ClickEvent, Context, Entity, EventEmitter, FontWeight, InteractiveElement as _, IntoElement, KeyDownEvent,
    ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _, Subscription, Window, div, px, rgb,
    component::input::{Input, InputEvent, InputState},
};
use gpui_kit::AppContext as _;
use oxicord_model::{ChannelType, Snowflake};

use crate::{
    admin::AdminRequest,
    backend::Backend,
    profile::modal,
    session::Session,
    theme,
};

pub struct CloseCreateChannel;

impl EventEmitter<CloseCreateChannel> for CreateChannelDialog {}

struct TypeOption {
    kind: ChannelType,
    label: &'static str,
    blurb: &'static str,
}

const TYPES: &[TypeOption] = &[
    TypeOption { kind: ChannelType::GuildText, label: "Text", blurb: "Send messages, images, GIFs, emoji, opinions, and puns" },
    TypeOption { kind: ChannelType::GuildVoice, label: "Voice", blurb: "Hang out together with voice, video, and screen share" },
    TypeOption { kind: ChannelType::GuildForum, label: "Forum", blurb: "Create a space for organized discussions" },
];

pub struct CreateChannelDialog {
    session: Entity<Session>,
    backend: Backend,
    guild: Option<Snowflake>,
    parent: Option<Snowflake>,
    name: Entity<InputState>,
    kind: ChannelType,
    _subscriptions: Vec<Subscription>,
}

impl CreateChannelDialog {
    pub fn new(session: Entity<Session>, backend: Backend, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let name = cx.new(|cx| InputState::new(window, cx).placeholder("new-channel"));
        let subscriptions = vec![
            cx.observe(&session, |_, _, cx| cx.notify()),
            cx.subscribe(&name, |this, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::PressEnter { .. }) {
                    this.save(cx);
                }
            }),
        ];
        Self { session, backend, guild: None, parent: None, name, kind: ChannelType::GuildText, _subscriptions: subscriptions }
    }

    /// Opens for `guild`, filing the channel under `parent` (a category) when set.
    pub fn opened(&mut self, guild: Snowflake, parent: Option<Snowflake>, window: &mut Window, cx: &mut Context<Self>) {
        self.guild = Some(guild);
        self.parent = parent;
        self.kind = ChannelType::GuildText;
        self.name.update(cx, |input, cx| {
            input.set_value(String::new(), window, cx);
            input.focus(window, cx);
        });
        cx.notify();
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        let (Some(guild), name) = (self.guild, self.name.read(cx).value().trim().to_owned()) else { return };
        if name.is_empty() {
            return;
        }
        let kind = self.kind as u8;
        let parent = self.parent;
        self.session.update(cx, |session, cx| {
            session.request_admin(
                &self.backend,
                AdminRequest::CreateChannel { guild, name, kind, parent },
                cx,
            );
        });
        cx.emit(CloseCreateChannel);
    }
}

impl Render for CreateChannelDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let name = self.name.read(cx).value().trim().to_owned();
        let can_create = !name.is_empty();

        let card = div()
            .id("create-channel-card")
            .occlude()
            .w(px(460.))
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
                    cx.emit(CloseCreateChannel);
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
                            .child("Create Channel"),
                    )
                    .child(
                        div()
                            .id("create-channel-close")
                            .cursor_pointer()
                            .text_color(rgb(theme::TEXT_MUTED))
                            .on_click(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(CloseCreateChannel)))
                            .child("✕"),
                    ),
            )
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(rgb(theme::TEXT_MUTED))
                    .child(match self.parent {
                        Some(_) => "in the category",
                        None => "in the server",
                    }),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(div().text_size(px(12.)).text_color(rgb(theme::TEXT_MUTED)).child("CHANNEL TYPE"))
                    .children(TYPES.iter().map(|opt| {
                        let selected = self.kind == opt.kind;
                        div()
                            .id(("type", opt.kind as u64))
                            .flex()
                            .items_center()
                            .gap(px(12.))
                            .p(px(12.))
                            .rounded(px(theme::RADIUS))
                            .cursor_pointer()
                            .bg(rgb(if selected { theme::BG_HOVER } else { theme::BG_PANEL }))
                            .border_1()
                            .border_color(rgb(if selected { theme::ACCENT } else { theme::BORDER_MUTED }))
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.kind = opt.kind;
                                cx.notify();
                            }))
                            .child(
                                div()
                                    .size(px(16.))
                                    .rounded_full()
                                    .border_2()
                                    .border_color(rgb(if selected { theme::ACCENT } else { theme::TEXT_MUTED }))
                                    .child(if selected {
                                        div().m(px(3.)).size(px(8.)).rounded_full().bg(rgb(theme::ACCENT)).into_any_element()
                                    } else {
                                        div().into_any_element()
                                    }),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .child(div().text_size(px(14.)).font_weight(FontWeight::MEDIUM).text_color(rgb(theme::TEXT)).child(opt.label))
                                    .child(div().text_size(px(12.)).text_color(rgb(theme::TEXT_MUTED)).child(opt.blurb)),
                            )
                    })),
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
                    .justify_end()
                    .gap(px(8.))
                    .child(
                        div()
                            .id("create-channel-cancel")
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
                            .on_click(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(CloseCreateChannel)))
                            .child("Cancel"),
                    )
                    .child(
                        div()
                            .id("create-channel-save")
                            .h(px(40.))
                            .px(px(20.))
                            .flex()
                            .items_center()
                            .rounded(px(theme::RADIUS))
                            .cursor_pointer()
                            .text_size(px(14.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgb(0xffffff))
                            .bg(rgb(theme::ACCENT))
                            .opacity(if can_create { 1. } else { 0.5 })
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                if !this.name.read(cx).value().trim().is_empty() {
                                    this.save(cx);
                                }
                            }))
                            .child("Create Channel"),
                    ),
            );

        modal(card.into_any_element(), Box::new(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(CloseCreateChannel))))
    }
}
