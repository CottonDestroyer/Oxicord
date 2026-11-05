use std::{
    collections::{HashMap, HashSet},
    rc::Rc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use chrono::{Local, NaiveDate, Utc};
use gpui_kit::{
    AnyElement, App, AppContext as _, ClickEvent, Context, Entity, FollowMode, FontWeight,
    Focusable as _,
    InteractiveElement as _, IntoElement, ListAlignment, ListState, ParentElement as _, Render,
    SharedString, StatefulInteractiveElement as _, Styled as _, Subscription, Task, Window, div,
    list, px, rgb,
    component::input::{InputEvent, Textarea, TextareaState},
};
use oxicord_markdown::Block;
use oxicord_media::cdn;
use oxicord_model::{Message, MessageType, Permissions, Snowflake};
use oxicord_store::Changes;

use super::{
    format,
    markdown::{self, Names, Options, SpoilerHandler},
    rows::{Row, build_rows, diff},
    typing::typing_text,
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

/// The message area for whichever channel is open: header, list, composer.
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

    input: Entity<TextareaState>,
    /// Unsent text per channel, so switching away doesn't lose it.
    drafts: HashMap<Snowflake, String>,
    reply_to: Option<Snowflake>,
    editing: Option<Snowflake>,
    /// The message whose delete button was pressed once already.
    confirm_delete: Option<Snowflake>,
    /// Spoilers the reader has clicked open, as (message, index in message).
    revealed: HashSet<(Snowflake, usize)>,

    /// The newest message we have acknowledged per channel, so we don't ack
    /// the same one twice.
    acked: HashMap<Snowflake, Snowflake>,
    /// Where the unread messages began when this channel was opened. It stays
    /// put while we read, so the "new" line doesn't move under the reader.
    unread_after: Option<Snowflake>,
    last_typing_sent: Option<Instant>,
    /// Whether the typing line is on screen, which decides if the clock needs
    /// to keep redrawing it.
    typing_shown: bool,
    _ticker: Task<()>,
    _subscriptions: Vec<Subscription>,
}

impl ChatView {
    pub fn new(
        session: Entity<Session>,
        media: Entity<Media>,
        backend: Backend,
        window: &mut Window,
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

        let input = cx.new(|cx| {
            TextareaState::new(window, cx)
                .auto_grow(1, 10)
                .submit_on_enter(true)
                .placeholder("Message")
        });

        let subscriptions = vec![
            cx.subscribe(&session, |this, _, changes: &Changes, cx| {
                this.on_changes(changes, cx)
            }),
            cx.observe(&session, |_, _, cx| cx.notify()),
            cx.observe(&media, |_, _, cx| cx.notify()),
            cx.subscribe_in(&input, window, |this, _, event: &InputEvent, window, cx| match event {
                InputEvent::PressEnter { shift: false, .. } => this.submit(window, cx),
                InputEvent::Change => this.note_typing(window, cx),
                _ => {}
            }),
        ];

        // A typing indicator has to disappear on its own, so redraw once a
        // second while one is showing.
        let ticker = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                let alive = this.update(cx, |this, cx| {
                    if this.typing_shown {
                        cx.notify();
                    }
                });
                if alive.is_err() {
                    break;
                }
            }
        });

        Self {
            session,
            media,
            backend,
            channel: None,
            list,
            rows: Vec::new(),
            fetched: HashSet::new(),
            loading: false,
            input,
            drafts: HashMap::new(),
            reply_to: None,
            editing: None,
            confirm_delete: None,
            revealed: HashSet::new(),
            acked: HashMap::new(),
            unread_after: None,
            last_typing_sent: None,
            typing_shown: false,
            _ticker: ticker,
            _subscriptions: subscriptions,
        }
    }

    pub fn set_channel(
        &mut self,
        channel: Option<Snowflake>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.stash_draft(cx);
        self.channel = channel;
        self.loading = false;
        self.reply_to = None;
        self.editing = None;
        self.confirm_delete = None;

        let draft = channel
            .and_then(|c| self.drafts.get(&c).cloned())
            .unwrap_or_default();
        self.input.update(cx, |input, cx| input.set_value(draft, window, cx));

        // Remember where the unread part starts before reading it clears it.
        self.unread_after = channel.and_then(|c| {
            let store = &self.session.read(cx).store;
            store.unread(c).unread.then(|| store.last_read(c)).flatten()
        });

        if let Some(id) = channel {
            self.session.update(cx, |s, _| s.store.open_channel(id));
            if self.fetched.insert(id) {
                self.request(id, None, cx);
            }
        }
        self.rows.clear();
        self.list.reset(0);
        self.rebuild(cx);
        self.maybe_ack(cx);
        cx.notify();
    }

    /// Marks the newest message read once we are looking at the end of the
    /// channel, here and on the server, so its badge goes away.
    fn maybe_ack(&mut self, cx: &mut Context<Self>) {
        let Some(channel) = self.channel else { return };
        let (newest, unread) = {
            let store = &self.session.read(cx).store;
            (
                store.timeline(channel).and_then(|t| t.newest_id()),
                store.unread(channel).unread,
            )
        };
        let Some(newest) = newest else { return };

        let already = self.acked.get(&channel).is_some_and(|a| *a >= newest);
        let reading_history = self.list.is_scrolled_to_end() == Some(false);
        if !unread || already || reading_history {
            return;
        }

        self.acked.insert(channel, newest);
        self.backend.send(Command::AckMessage {
            channel,
            message: newest,
        });
        self.session.update(cx, |session, cx| {
            let changes = session.store.mark_read(channel, newest);
            cx.emit(changes);
            cx.notify();
        });
    }

    /// Tells the channel we are typing, at most once every few seconds and
    /// only for text the user is actually entering, not a restored draft.
    fn note_typing(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        const EVERY: Duration = Duration::from_secs(8);

        let Some(channel) = self.channel else { return };
        let focused = self.input.read(cx).focus_handle(cx).is_focused(window);
        let has_text = !self.input.read(cx).value().trim().is_empty();
        if !focused || !has_text || self.editing.is_some() {
            return;
        }
        if self.last_typing_sent.is_some_and(|at| at.elapsed() < EVERY) {
            return;
        }
        self.last_typing_sent = Some(Instant::now());
        self.backend.send(Command::Typing(channel));
    }

    /// Remembers what was typed in the channel we are leaving.
    fn stash_draft(&mut self, cx: &App) {
        let Some(channel) = self.channel else { return };
        // Text typed while editing belongs to the edit, not to the draft.
        if self.editing.is_some() {
            return;
        }
        let text = self.input.read(cx).value().to_string();
        if text.trim().is_empty() {
            self.drafts.remove(&channel);
        } else {
            self.drafts.insert(channel, text);
        }
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
            // A new session wipes the store. Forget what we fetched and
            // reopen the channel once the window is next available.
            self.fetched.clear();
            self.session.update(cx, |s, _| s.store.open_channel(channel));
            self.fetched.insert(channel);
            self.request(channel, None, cx);
            self.rows.clear();
            self.list.reset(0);
            self.rebuild(cx);
            return;
        }
        if changes.messages.contains(&channel) {
            self.loading = false;
            self.rebuild(cx);
            self.maybe_ack(cx);
            cx.notify();
        }
        if changes.typing.contains(&channel) {
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
            .map(|t| build_rows(t.messages(), offset, self.unread_after))
            .unwrap_or_default();

        match diff(&self.rows, &new) {
            Some((range, count)) => self.list.splice(range, count),
            // Same rows, but a message may have been edited or reacted to,
            // which changes its height.
            None => self.list.remeasure(),
        }
        self.rows = new;
    }

    fn message(&self, id: Snowflake, cx: &App) -> Option<Message> {
        let channel = self.channel?;
        self.session
            .read(cx)
            .store
            .timeline(channel)?
            .get(id)
            .cloned()
    }

    fn permissions(&self, cx: &App) -> Permissions {
        let store = &self.session.read(cx).store;
        self.channel
            .and_then(|c| store.channel(c))
            .map(|c| store.permissions_in(c))
            .unwrap_or_default()
    }

    fn guild(&self, cx: &App) -> Option<Snowflake> {
        let store = &self.session.read(cx).store;
        self.channel.and_then(|c| store.channel(c)).and_then(|c| c.guild_id)
    }

    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(channel) = self.channel else { return };
        if !self.permissions(cx).contains(Permissions::SEND_MESSAGES) {
            return;
        }
        let text = self.input.read(cx).value().trim().to_owned();
        if text.is_empty() {
            return;
        }

        match self.editing.take() {
            Some(message) => self.backend.send(Command::EditMessage {
                channel,
                message,
                content: text,
            }),
            None => {
                let guild = self.guild(cx);
                self.backend.send(Command::SendMessage {
                    channel,
                    guild,
                    content: text,
                    reply_to: self.reply_to.take(),
                });
            }
        }
        self.drafts.remove(&channel);
        self.input.update(cx, |input, cx| input.set_value("", window, cx));
        cx.notify();
    }

    fn start_reply(&mut self, message: Snowflake, window: &mut Window, cx: &mut Context<Self>) {
        self.reply_to = Some(message);
        self.editing = None;
        self.input.update(cx, |input, cx| input.focus(window, cx));
        cx.notify();
    }

    fn start_edit(&mut self, message: Snowflake, window: &mut Window, cx: &mut Context<Self>) {
        let Some(original) = self.message(message, cx) else { return };
        self.stash_draft(cx);
        self.editing = Some(message);
        self.reply_to = None;
        self.input.update(cx, |input, cx| {
            input.set_value(original.content, window, cx);
            input.focus(window, cx);
        });
        cx.notify();
    }

    fn cancel_compose(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let was_editing = self.editing.take().is_some();
        self.reply_to = None;
        if was_editing {
            let draft = self
                .channel
                .and_then(|c| self.drafts.get(&c).cloned())
                .unwrap_or_default();
            self.input.update(cx, |input, cx| input.set_value(draft, window, cx));
        }
        cx.notify();
    }

    fn delete(&mut self, message: Snowflake, cx: &mut Context<Self>) {
        if let Some(channel) = self.channel {
            self.backend.send(Command::DeleteMessage { channel, message });
        }
        self.confirm_delete = None;
        cx.notify();
    }

    fn toggle_spoiler(&mut self, message: Snowflake, index: usize, cx: &mut Context<Self>) {
        self.revealed.insert((message, index));
        cx.notify();
    }

    /// The message text, drawn as markdown.
    fn render_content(
        &mut self,
        message: &Message,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let blocks = oxicord_markdown::parse(&message.content);
        let names = self.names_for(message, &blocks, cx);
        let revealed: HashSet<usize> = self
            .revealed
            .iter()
            .filter(|(id, _)| *id == message.id)
            .map(|(_, index)| *index)
            .collect();

        let (view, id) = (cx.weak_entity(), message.id);
        let on_spoiler: SpoilerHandler =
            Rc::new(move |index, _, cx| {
                view.update(cx, |this, cx| this.toggle_spoiler(id, index, cx)).ok();
            });

        let options = Options {
            names: &names,
            revealed: &revealed,
            media: &self.media,
            now: Utc::now(),
            offset: *Local::now().offset(),
            edited: message.edited_timestamp.is_some(),
            key: id.0,
            on_spoiler,
        };
        markdown::render(&blocks, &options, window, cx)
    }

    /// Looks up the names a message's mentions refer to.
    fn names_for(&self, message: &Message, blocks: &[Block], cx: &App) -> Names {
        let ids = markdown::mention_ids(blocks);
        let store = &self.session.read(cx).store;
        let guild = self
            .channel
            .and_then(|c| store.channel(c))
            .and_then(|c| c.guild_id)
            .and_then(|g| store.guild(g));

        let mut names = Names::default();
        for id in ids.users {
            let name = message
                .mentions
                .iter()
                .find(|u| u.id == id)
                .map(|u| u.display_name().to_owned())
                .or_else(|| store.user(id).map(|u| u.display_name().to_owned()));
            if let Some(name) = name {
                names.users.insert(id, name);
            }
        }
        for id in ids.roles {
            if let Some(role) = guild.and_then(|g| g.roles.iter().find(|r| r.id == id)) {
                names.roles.insert(id, (role.name.clone(), role.color));
            }
        }
        for id in ids.channels {
            if let Some(name) = store.channel(id).and_then(|c| c.name.clone()) {
                names.channels.insert(id, name);
            }
        }
        names
    }

    fn render_row(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        match self.rows.get(ix).cloned() {
            Some(Row::Divider(day)) => divider(day).into_any_element(),
            Some(Row::NewMessages) => new_messages_line().into_any_element(),
            Some(Row::Message { id, grouped }) => match self.message(id, cx) {
                Some(message) => self.render_message(message, grouped, window, cx),
                None => div().into_any_element(),
            },
            None => div().into_any_element(),
        }
    }

    fn render_message(
        &mut self,
        message: Message,
        grouped: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let offset = *Local::now().offset();
        let id = message.id;

        if !message.kind.is_regular() {
            return system_row(&message, &self.media, cx);
        }

        let (is_mine, perms) = {
            let me = self.session.read(cx).store.me().map(|u| u.id);
            (me == Some(message.author.id), self.permissions(cx))
        };
        let can_reply = perms.contains(Permissions::SEND_MESSAGES);
        let can_delete = is_mine || perms.contains(Permissions::MANAGE_MESSAGES);

        let author = message.author.display_name().to_owned();
        let url = cdn::user_avatar(message.author.id, message.author.avatar.as_deref(), 64);

        let left: AnyElement = if grouped {
            div().w(px(GUTTER)).flex_none().into_any_element()
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
        body = body.child(self.render_content(&message, window, cx));

        let confirming = self.confirm_delete == Some(id);
        let bar = self.action_bar(id, can_reply, is_mine, can_delete, confirming, cx);

        div()
            .id(("message", id.0))
            .group("message")
            .relative()
            .w_full()
            .flex()
            .gap_4()
            .px_4()
            .pt(px(if grouped { 1. } else { 16. }))
            .pb(px(1.))
            .hover(|s| s.bg(rgb(theme::BG_HOVER)))
            .child(left)
            .child(body)
            .child(bar)
            .into_any_element()
    }

    /// Reply, edit and delete, floating at the top right of a hovered
    /// message. While a delete is waiting for confirmation it stays visible.
    fn action_bar(
        &mut self,
        id: Snowflake,
        can_reply: bool,
        can_edit: bool,
        can_delete: bool,
        confirming: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let mut bar = div()
            .absolute()
            .right(px(16.))
            .top(px(-12.))
            .flex()
            .items_center()
            .gap(px(2.))
            .p(px(2.))
            .rounded(px(6.))
            .border_1()
            .border_color(rgb(theme::BORDER))
            .bg(rgb(theme::BG_SIDEBAR));

        if !confirming {
            bar = bar.invisible().group_hover("message", |s| s.visible());
        }

        if confirming {
            return bar
                .child(
                    div()
                        .id(("confirm-delete", id.0))
                        .px_2()
                        .rounded(px(4.))
                        .cursor_pointer()
                        .bg(rgb(theme::BADGE))
                        .text_size(px(12.))
                        .text_color(rgb(theme::TEXT_BRIGHT))
                        .on_click(cx.listener(move |this, _, _, cx| this.delete(id, cx)))
                        .child("Delete"),
                )
                .child(
                    action_button(("cancel-delete", id.0), "x", theme::TEXT_MUTED).on_click(
                        cx.listener(|this, _, _, cx| {
                            this.confirm_delete = None;
                            cx.notify();
                        }),
                    ),
                );
        }

        if can_reply {
            bar = bar.child(
                action_button(("reply", id.0), "reply", theme::TEXT_MUTED).on_click(
                    cx.listener(move |this, _, window, cx| this.start_reply(id, window, cx)),
                ),
            );
        }
        if can_edit {
            bar = bar.child(
                action_button(("edit", id.0), "pencil", theme::TEXT_MUTED).on_click(
                    cx.listener(move |this, _, window, cx| this.start_edit(id, window, cx)),
                ),
            );
        }
        if can_delete {
            bar = bar.child(
                action_button(("delete", id.0), "trash", theme::DANGER).on_click(cx.listener(
                    move |this, _: &ClickEvent, _, cx| {
                        this.confirm_delete = Some(id);
                        cx.notify();
                    },
                )),
            );
        }
        bar
    }

    fn render_header(&self, cx: &App) -> impl IntoElement + use<> {
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

        list(self.list.clone(), cx.processor(|this, ix, window, cx| this.render_row(ix, window, cx)))
            .size_full()
            .into_any_element()
    }

    /// The notice banner, the reply or edit bar, and the text box.
    fn render_composer(&mut self, cx: &mut Context<Self>) -> AnyElement {
        if self.channel.is_none() {
            return div().into_any_element();
        }

        let notice = self.session.read(cx).notice.clone();
        let can_send = self.permissions(cx).contains(Permissions::SEND_MESSAGES);
        let typing = self.typing_line(cx);
        self.typing_shown = typing.is_some();

        let mut column = div().flex_none().flex().flex_col().px_4().pb_2().pt_1();

        // The line always takes up its height so the composer doesn't jump
        // when someone starts or stops typing.
        column = column.child(
            div()
                .h(px(18.))
                .text_size(px(12.))
                .text_color(rgb(theme::TEXT_MUTED))
                .child(typing.unwrap_or_default()),
        );

        if let Some(text) = notice {
            column = column.child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .mb_2()
                    .px_3()
                    .py_2()
                    .rounded(px(6.))
                    .bg(rgb(0x3a2326))
                    .text_size(px(13.))
                    .text_color(rgb(theme::DANGER))
                    .child(div().flex_1().child(text))
                    .child(
                        action_button("dismiss-notice", "x", theme::DANGER).on_click(cx.listener(
                            |this, _, _, cx| {
                                this.session.update(cx, |s, cx| s.set_notice(None, cx));
                            },
                        )),
                    ),
            );
        }

        if !can_send {
            return column
                .child(
                    div()
                        .px_4()
                        .py_3()
                        .rounded(px(8.))
                        .bg(rgb(theme::BG_INPUT))
                        .text_color(rgb(theme::TEXT_FAINT))
                        .child("You do not have permission to send messages in this channel."),
                )
                .into_any_element();
        }

        if let Some(bar) = self.compose_context_bar(cx) {
            column = column.child(bar);
        }

        column
            .child(
                div()
                    .flex()
                    .items_end()
                    .gap_2()
                    .px_3()
                    .py_2()
                    .rounded(px(8.))
                    .bg(rgb(theme::BG_INPUT))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(Textarea::new(&self.input).appearance(false).bordered(false)),
                    )
                    .child(
                        action_button("send", "send", theme::TEXT_MUTED).on_click(cx.listener(
                            |this, _, window, cx| this.submit(window, cx),
                        )),
                    ),
            )
            .into_any_element()
    }

    /// "Ada is typing...", or nothing.
    fn typing_line(&self, cx: &App) -> Option<String> {
        let channel = self.channel?;
        let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();
        let names: Vec<String> = self
            .session
            .read(cx)
            .store
            .typing_users(channel, now)
            .into_iter()
            .map(|u| u.display_name().to_owned())
            .collect();
        typing_text(&names)
    }

    /// "Replying to X" or "Editing message", with a button to cancel.
    fn compose_context_bar(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let label: SharedString = if self.editing.is_some() {
            "Editing message".into()
        } else {
            let reply = self.reply_to?;
            let name = self
                .message(reply, cx)
                .map(|m| m.author.display_name().to_owned())
                .unwrap_or_else(|| "message".into());
            format!("Replying to {name}").into()
        };

        Some(
            div()
                .flex()
                .items_center()
                .gap_2()
                .px_3()
                .py_1()
                .rounded_t(px(8.))
                .bg(rgb(theme::BG_SIDEBAR))
                .text_size(px(13.))
                .text_color(rgb(theme::TEXT_MUTED))
                .child(div().flex_1().child(label))
                .child(
                    action_button("cancel-compose", "x", theme::TEXT_MUTED).on_click(cx.listener(
                        |this, _, window, cx| this.cancel_compose(window, cx),
                    )),
                )
                .into_any_element(),
        )
    }
}

impl Render for ChatView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let header = self.render_header(cx);
        let body = self.render_body(cx);
        let composer = self.render_composer(cx);

        div()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .flex_col()
            .bg(rgb(theme::BG_CHAT))
            .child(header)
            .child(div().flex_1().min_h_0().child(body))
            .child(composer)
    }
}

/// A small square icon button. The caller attaches the click handler.
fn action_button(
    id: impl Into<gpui_kit::ElementId>,
    glyph: &str,
    color: u32,
) -> gpui_kit::Stateful<gpui_kit::Div> {
    div()
        .id(id)
        .size(px(28.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(4.))
        .cursor_pointer()
        .hover(|s| s.bg(rgb(theme::BG_HOVER)))
        .child(icon(glyph, 16., color))
}

/// The red line above the first message you haven't read.
fn new_messages_line() -> impl IntoElement {
    div()
        .w_full()
        .flex()
        .items_center()
        .px_4()
        .mt_2()
        .child(div().flex_1().h(px(1.)).bg(rgb(theme::BADGE)))
        .child(
            div()
                .px(px(6.))
                .rounded(px(4.))
                .bg(rgb(theme::BADGE))
                .text_size(px(10.))
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(theme::TEXT_BRIGHT))
                .child("NEW"),
        )
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

/// Joins, pins and other notices the server posts as messages.
fn system_row(message: &Message, media: &Entity<Media>, cx: &mut App) -> AnyElement {
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
