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
use oxicord_model::{ChannelType, Snowflake};

use crate::{
    admin::AdminRequest,
    backend::{Backend, Command},
    channel_settings::OpenChannelPermissions,
    icons::icon,
    profile::modal,
    session::Session,
    theme,
};

pub struct CloseEditChannel;

impl EventEmitter<CloseEditChannel> for EditChannelDialog {}

/// Which of the follow pickers is open.
#[derive(Clone, Copy, PartialEq, Eq)]
enum FollowList {
    Servers,
    Channels,
}

pub struct EditChannelDialog {
    session: Entity<Session>,
    backend: Backend,
    channel: Option<Snowflake>,
    name: Entity<InputState>,
    topic: Entity<InputState>,
    follow_guild: Option<Snowflake>,
    follow_channel: Option<Snowflake>,
    follow_open: Option<FollowList>,
    _subscriptions: Vec<Subscription>,
}

impl EditChannelDialog {
    pub fn new(session: Entity<Session>, backend: Backend, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let name = cx.new(|cx| InputState::new(window, cx).placeholder("Channel name"));
        let topic = cx.new(|cx| InputState::new(window, cx).placeholder("Add a topic"));
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
        Self {
            session,
            backend,
            channel: None,
            name,
            topic,
            follow_guild: None,
            follow_channel: None,
            follow_open: None,
            _subscriptions: subscriptions,
        }
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
        self.follow_guild = None;
        self.follow_channel = None;
        self.follow_open = None;
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

fn picker_head(
    id: &'static str,
    label: String,
    enabled: bool,
) -> gpui_kit::Stateful<gpui_kit::Div> {
    let head = div()
        .id(id)
        .h(px(40.))
        .px(px(12.))
        .flex()
        .items_center()
        .justify_between()
        .rounded(px(theme::RADIUS))
        .border_1()
        .border_color(rgb(theme::BORDER))
        .bg(rgb(theme::BG_INPUT))
        .text_size(px(14.))
        .text_color(rgb(if enabled { theme::TEXT } else { theme::TEXT_MUTED }))
        .child(div().truncate().child(label))
        .child(icon("chevron-down", 16., theme::TEXT_MUTED));
    if enabled { head.cursor_pointer() } else { head }
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

        let (guilds, channels) = {
            let store = &self.session.read(cx).store;
            let guilds: Vec<(Snowflake, String)> = store
                .guilds()
                .map(|guild| (guild.id, guild.name.clone()))
                .collect();
            let channels: Vec<(Snowflake, String)> = self
                .follow_guild
                .map(|guild| {
                    store
                        .channel_groups(guild)
                        .iter()
                        .flat_map(|group| group.channels.iter())
                        .filter(|channel| {
                            matches!(
                                channel.kind,
                                ChannelType::GuildText | ChannelType::GuildAnnouncement
                            )
                        })
                        .map(|channel| (channel.id, channel.name.clone().unwrap_or_default()))
                        .collect()
                })
                .unwrap_or_default();
            (guilds, channels)
        };
        let server_name = self
            .follow_guild
            .and_then(|id| guilds.iter().find(|(guild, _)| *guild == id))
            .map(|(_, name)| name.clone())
            .unwrap_or_else(|| "Choose a server".to_owned());
        let channel_name = self
            .follow_channel
            .and_then(|id| channels.iter().find(|(channel, _)| *channel == id))
            .map(|(_, name)| name.clone())
            .unwrap_or_else(|| "Choose a channel".to_owned());
        let can_follow = self.follow_guild.is_some() && self.follow_channel.is_some();

        let server_head =
            picker_head("follow-server", server_name, true).on_click(cx.listener(
                |this, _: &ClickEvent, _, cx| {
                    this.follow_open = (this.follow_open != Some(FollowList::Servers))
                        .then_some(FollowList::Servers);
                    cx.notify();
                },
            ));
        let channel_head =
            picker_head("follow-channel", channel_name, self.follow_guild.is_some()).on_click(
                cx.listener(|this, _: &ClickEvent, _, cx| {
                    if this.follow_guild.is_none() {
                        return;
                    }
                    this.follow_open = (this.follow_open != Some(FollowList::Channels))
                        .then_some(FollowList::Channels);
                    cx.notify();
                }),
            );

        let mut follow_form = div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(rgb(theme::TEXT_MUTED))
                    .child("FOLLOW TARGET CHANNEL"),
            )
            .child(server_head);

        if self.follow_open == Some(FollowList::Servers) {
            let mut list = div()
                .id("follow-servers")
                .max_h(px(200.))
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .rounded(px(theme::RADIUS))
                .border_1()
                .border_color(rgb(theme::BORDER))
                .bg(rgb(theme::BG_PANEL));
            for (index, (guild, name)) in guilds.into_iter().enumerate() {
                let picked = self.follow_guild == Some(guild);
                list = list.child(
                    div()
                        .id(("follow-server", index))
                        .h(px(32.))
                        .px_2()
                        .flex()
                        .items_center()
                        .rounded(px(4.))
                        .cursor_pointer()
                        .text_size(px(14.))
                        .text_color(rgb(theme::TEXT))
                        .bg(rgb(if picked { theme::BG_ACTIVE } else { theme::BG_PANEL }))
                        .hover(|s| s.bg(rgb(theme::BG_HOVER)))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.follow_guild = Some(guild);
                            this.follow_channel = None;
                            this.follow_open = Some(FollowList::Channels);
                            cx.notify();
                        }))
                        .child(name),
                );
            }
            follow_form = follow_form.child(list);
        }

        follow_form = follow_form.child(channel_head);

        if self.follow_open == Some(FollowList::Channels) && self.follow_guild.is_some() {
            let mut list = div()
                .id("follow-channels")
                .max_h(px(200.))
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .rounded(px(theme::RADIUS))
                .border_1()
                .border_color(rgb(theme::BORDER))
                .bg(rgb(theme::BG_PANEL));
            if channels.is_empty() {
                list = list.child(
                    div()
                        .px_2()
                        .py_2()
                        .text_size(px(13.))
                        .text_color(rgb(theme::TEXT_MUTED))
                        .child("No text channels in that server."),
                );
            }
            for (index, (channel, name)) in channels.into_iter().enumerate() {
                let picked = self.follow_channel == Some(channel);
                list = list.child(
                    div()
                        .id(("follow-channel", index))
                        .h(px(32.))
                        .px_2()
                        .flex()
                        .items_center()
                        .rounded(px(4.))
                        .cursor_pointer()
                        .text_size(px(14.))
                        .text_color(rgb(theme::TEXT))
                        .bg(rgb(if picked { theme::BG_ACTIVE } else { theme::BG_PANEL }))
                        .hover(|s| s.bg(rgb(theme::BG_HOVER)))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.follow_channel = Some(channel);
                            this.follow_open = None;
                            cx.notify();
                        }))
                        .child(name),
                );
            }
            follow_form = follow_form.child(list);
        }

        let follow_button = if can_follow {
            button("follow-channel", "Follow", true).on_click(cx.listener(
                |this, _: &ClickEvent, _, cx| {
                    let (Some(source), Some(target)) = (this.channel, this.follow_channel) else {
                        return;
                    };
                    this.backend.send(Command::FollowChannel { source, target });
                    this.follow_open = None;
                    cx.notify();
                },
            ))
        } else {
            button("follow-channel", "Follow", true).opacity(0.5)
        };
        follow_form = follow_form.child(follow_button);

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
            .child(follow_form)
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(8.))
                    .child(
                        button("edit-channel-permissions", "Permissions", false).on_click(cx.listener(
                            |this, _: &ClickEvent, _, cx| {
                                if let Some(channel) = this.channel {
                                    this.session.update(cx, |_, cx| cx.emit(OpenChannelPermissions(channel, false)));
                                }
                                cx.emit(CloseEditChannel);
                            },
                        )),
                    )
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
