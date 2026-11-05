use std::collections::HashSet;

use chrono::{Local, NaiveDate, Utc};
use gpui_kit::{
    AnyElement, Context, Entity, FollowMode, FontWeight, InteractiveElement as _, IntoElement,
    ListAlignment, ListState, ParentElement as _, Render, SharedString,
    Styled as _, Subscription, Window, div, list, px, rgb,
};
use oxicord_media::cdn;
use oxicord_model::{Message, MessageType, Snowflake};
use oxicord_store::Changes;

use super::{
    format,
    rows::{Row, build_rows, diff},
};
use crate::{
    backend::{Backend, Command},
    icons::icon,
    media::{Media, avatar},
    session::Session,
    theme,
};

/// Width of the avatar column, which grouped messages leave empty so the
/// text lines up.
const GUTTER: f32 = 40.;

/// The message area for whichever channel is open: header, list, and (as
/// features arrive) the composer.
pub struct ChatView {
    session: Entity<Session>,
    media: Entity<Media>,
    backend: Backend,
    channel: Option<Snowflake>,
    list: ListState,
    rows: Vec<Row>,
    /// Channels whose first page of history has been requested this session.
    fetched: HashSet<Snowflake>,
    loading: bool,
    _subscriptions: Vec<Subscription>,
}

impl ChatView {
    pub fn new(
        session: Entity<Session>,
        media: Entity<Media>,
        backend: Backend,
        cx: &mut Context<Self>,
    ) -> Self {
        let list = ListState::new(0, ListAlignment::Bottom, px(800.));
        // Stay glued to the newest message while we are looking at it.
        list.set_follow_mode(FollowMode::Tail);

        let this = cx.weak_entity();
        list.set_scroll_handler(move |event, _, cx| {
            if event.visible_range.start < 4 {
                this.update(cx, |this, cx| this.load_older(cx)).ok();
            }
        });

        let subscriptions = vec![
            cx.subscribe(&session, |this, _, changes: &Changes, cx| {
                this.on_changes(changes, cx)
            }),
            cx.observe(&media, |_, _, cx| cx.notify()),
        ];

        Self {
            session,
            media,
            backend,
            channel: None,
            list,
            rows: Vec::new(),
            fetched: HashSet::new(),
            loading: false,
            _subscriptions: subscriptions,
        }
    }

    pub fn set_channel(&mut self, channel: Option<Snowflake>, cx: &mut Context<Self>) {
        self.channel = channel;
        self.loading = false;

        if let Some(id) = channel {
            self.session.update(cx, |s, _| s.store.open_channel(id));
            if self.fetched.insert(id) {
                self.request(id, None, cx);
            }
        }
        self.rows.clear();
        self.list.reset(0);
        self.rebuild(cx);
        cx.notify();
    }

    fn request(&mut self, channel: Snowflake, before: Option<Snowflake>, cx: &mut Context<Self>) {
        self.loading = true;
        self.backend.send(Command::LoadHistory { channel, before });
        cx.notify();
    }

    /// Asks for the page before the oldest message we hold, if there is one.
    fn load_older(&mut self, cx: &mut Context<Self>) {
        let Some(channel) = self.channel else { return };
        if self.loading || self.session.read(cx).history_errors.contains_key(&channel) {
            return;
        }
        let oldest = self
            .session
            .read(cx)
            .store
            .timeline(channel)
            .filter(|t| t.has_more_before())
            .and_then(|t| t.oldest_id());
        if let Some(oldest) = oldest {
            self.request(channel, Some(oldest), cx);
        }
    }

    fn on_changes(&mut self, changes: &Changes, cx: &mut Context<Self>) {
        let Some(channel) = self.channel else { return };

        if changes.full {
            // A new session wipes the store, so start the channel over.
            self.fetched.clear();
            self.set_channel(Some(channel), cx);
            return;
        }
        if changes.messages.contains(&channel) {
            self.loading = false;
            self.rebuild(cx);
            cx.notify();
        }
    }

    /// Brings the list in line with the store, touching only the rows that
    /// changed so the scroll position and measured heights survive.
    fn rebuild(&mut self, cx: &mut Context<Self>) {
        let offset = *Local::now().offset();
        let new = self
            .channel
            .and_then(|c| self.session.read(cx).store.timeline(c))
            .map(|t| build_rows(t.messages(), offset))
            .unwrap_or_default();

        match diff(&self.rows, &new) {
            Some((range, count)) => self.list.splice(range, count),
            // Same rows, but a message may have been edited or reacted to,
            // which changes its height.
            None => self.list.remeasure(),
        }
        self.rows = new;
    }

    fn message(&self, id: Snowflake, cx: &gpui_kit::App) -> Option<Message> {
        let channel = self.channel?;
        self.session
            .read(cx)
            .store
            .timeline(channel)?
            .get(id)
            .cloned()
    }

    fn render_row(&mut self, ix: usize, cx: &mut Context<Self>) -> AnyElement {
        match self.rows.get(ix).cloned() {
            Some(Row::Divider(day)) => divider(day).into_any_element(),
            Some(Row::Message { id, grouped }) => match self.message(id, cx) {
                Some(message) => self.render_message(message, grouped, cx),
                None => div().into_any_element(),
            },
            None => div().into_any_element(),
        }
    }

    fn render_message(&mut self, message: Message, grouped: bool, cx: &mut Context<Self>) -> AnyElement {
        let offset = *Local::now().offset();
        let id = message.id;

        if !message.kind.is_regular() {
            return system_row(&message, &self.media, cx);
        }

        let author = message.author.display_name().to_owned();
        let url = cdn::user_avatar(message.author.id, message.author.avatar.as_deref(), 64);

        let left: AnyElement = if grouped {
            div()
                .w(px(GUTTER))
                .flex_none()
                .into_any_element()
        } else {
            avatar(&self.media, &url, GUTTER, &author, cx).into_any_element()
        };

        let mut body = div().flex_1().min_w_0().flex().flex_col();
        if !grouped {
            body = body.child(
                div()
                    .flex()
                    .items_baseline()
                    .gap_2()
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(theme::TEXT_BRIGHT))
                            .child(author),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(rgb(theme::TEXT_FAINT))
                            .child(format::message_time(message.timestamp, Utc::now(), offset)),
                    ),
            );
        }
        if let Some(reply) = reply_line(&message) {
            body = body.child(reply);
        }
        body = body.child(content_text(&message));

        div()
            .id(("message", id.0))
            .w_full()
            .flex()
            .gap_4()
            .px_4()
            .pt(px(if grouped { 1. } else { 16. }))
            .pb(px(1.))
            .hover(|s| s.bg(rgb(theme::BG_HOVER)))
            .child(left)
            .child(body)
            .into_any_element()
    }

    fn render_header(&self, cx: &gpui_kit::App) -> impl IntoElement + use<> {
        let (glyph, name, topic) = self
            .channel
            .and_then(|id| {
                let store = &self.session.read(cx).store;
                let channel = store.channel(id)?;
                Some(if channel.kind.is_dm() {
                    ("at-sign", store.dm_name(channel), None)
                } else {
                    ("hash", channel.name.clone().unwrap_or_default(), channel.topic.clone())
                })
            })
            .unwrap_or(("hash", String::new(), None));

        div()
            .h(px(48.))
            .flex_none()
            .flex()
            .items_center()
            .gap_2()
            .px_4()
            .border_b_1()
            .border_color(rgb(theme::BORDER))
            .child(icon(glyph, 20., theme::TEXT_MUTED))
            .child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(theme::TEXT_BRIGHT))
                    .child(name),
            )
            .children(topic.filter(|t| !t.is_empty()).map(|topic| {
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .ml_2()
                    .pl_3()
                    .border_l_1()
                    .border_color(rgb(theme::BORDER))
                    .text_size(px(13.))
                    .text_color(rgb(theme::TEXT_MUTED))
                    .child(topic)
            }))
    }

    fn render_body(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let note = |text: SharedString| {
            div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_color(rgb(theme::TEXT_MUTED))
                .child(text)
                .into_any_element()
        };

        let Some(channel) = self.channel else {
            return note("Pick a channel to get started.".into());
        };

        if self.rows.is_empty() {
            let error = self.session.read(cx).history_errors.get(&channel).cloned();
            return note(match (error, self.loading) {
                (Some(error), _) => error.into(),
                (None, true) => "Loading messages...".into(),
                (None, false) => "No messages here yet.".into(),
            });
        }

        list(self.list.clone(), cx.processor(|this, ix, _, cx| this.render_row(ix, cx)))
            .size_full()
            .into_any_element()
    }
}

impl Render for ChatView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let header = self.render_header(cx);
        let body = self.render_body(cx);

        div()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .flex_col()
            .bg(rgb(theme::BG_CHAT))
            .child(header)
            .child(div().flex_1().min_h_0().child(body))
    }
}

fn divider(day: NaiveDate) -> impl IntoElement {
    let line = || div().flex_1().h(px(1.)).bg(rgb(theme::BORDER));

    div()
        .w_full()
        .flex()
        .items_center()
        .gap_2()
        .px_4()
        .mt_4()
        .mb_1()
        .child(line())
        .child(
            div()
                .text_size(px(12.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(theme::TEXT_MUTED))
                .child(format::day_label(day)),
        )
        .child(line())
}

/// The quoted line above a reply: who it answers and what they said.
fn reply_line(message: &Message) -> Option<AnyElement> {
    if message.kind != MessageType::Reply && message.referenced_message.is_none() {
        return None;
    }

    let content = match &message.referenced_message {
        Some(original) => div()
            .flex()
            .items_center()
            .gap_1()
            .min_w_0()
            .child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(format!("@{}", original.author.display_name())),
            )
            .child(div().truncate().child(snippet(&original.content)))
            .into_any_element(),
        None => div()
            .italic()
            .child("Original message was deleted")
            .into_any_element(),
    };

    Some(
        div()
            .flex()
            .items_center()
            .gap_2()
            .mb_1()
            .text_size(px(13.))
            .text_color(rgb(theme::TEXT_MUTED))
            .child(icon("reply", 14., theme::TEXT_MUTED))
            .child(content)
            .into_any_element(),
    )
}

/// One line of a quoted message, cut short.
fn snippet(content: &str) -> String {
    let line = content.lines().next().unwrap_or_default();
    if line.is_empty() {
        "Click to see attachment".to_owned()
    } else {
        line.to_owned()
    }
}

fn content_text(message: &Message) -> impl IntoElement + use<> {
    let edited = message.edited_timestamp.is_some();

    div()
        .text_color(rgb(theme::TEXT))
        .child(message.content.clone())
        .children(edited.then(|| {
            div()
                .text_size(px(11.))
                .text_color(rgb(theme::TEXT_FAINT))
                .child("(edited)")
        }))
}

/// Joins, pins and other notices the server posts as messages.
fn system_row(message: &Message, media: &Entity<Media>, cx: &mut gpui_kit::App) -> AnyElement {
    let name = message.author.display_name().to_owned();
    let url = cdn::user_avatar(message.author.id, message.author.avatar.as_deref(), 32);
    let text = match message.kind {
        MessageType::UserJoin => format!("{name} joined the server."),
        MessageType::ChannelPinnedMessage => format!("{name} pinned a message to this channel."),
        MessageType::RecipientAdd => format!("{name} added someone to the group."),
        MessageType::RecipientRemove => format!("{name} left the group."),
        MessageType::Call => format!("{name} started a call."),
        MessageType::ChannelNameChange => format!("{name} changed the channel name."),
        MessageType::ChannelIconChange => format!("{name} changed the channel icon."),
        _ => format!("{name} sent a message Oxicord can't show yet."),
    };

    div()
        .id(("system", message.id.0))
        .w_full()
        .flex()
        .items_center()
        .gap_4()
        .px_4()
        .py_1()
        .text_size(px(14.))
        .text_color(rgb(theme::TEXT_MUTED))
        .child(
            div()
                .w(px(GUTTER))
                .flex_none()
                .flex()
                .justify_center()
                .child(avatar(media, &url, 24., &name, cx)),
        )
        .child(div().child(text))
        .into_any_element()
}
