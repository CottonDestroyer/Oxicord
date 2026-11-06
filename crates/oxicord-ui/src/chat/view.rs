use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    rc::Rc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use chrono::{Local, NaiveDate, Utc};
use gpui_kit::{
    AnyElement, App, AppContext as _, ClickEvent, Context, Entity, ExternalPaths, FollowMode, FontWeight,
    KeyDownEvent, MouseButton, MouseDownEvent,
    PathPromptOptions,
    Focusable as _,
    InteractiveElement as _, IntoElement, ListAlignment, ListState, ParentElement as _, Render,
    SharedString, StatefulInteractiveElement as _, Styled as _, Subscription, Task, Window, div,
    list, px, rgb,
    component::input::{Escape, InputEvent, Textarea, TextareaState},
};
use oxicord_markdown::Block;
use oxicord_media::cdn;
use oxicord_model::{ChannelType, Emoji, Message, MessageType, Permissions, Snowflake};
use oxicord_settings::Key;
use oxicord_store::Changes;

use super::{
    attachments::{render_attachments, render_embeds},
    format, intro,
    markdown::{self, Names, Options, SpoilerHandler},
    rows::{Row, build_rows, diff},
    typing::typing_text,
};
use crate::{
    backend::{Backend, Command},
    emoji_panel::{ClosePicker, EmojiPanel, Picked, PickedGif, PickedSticker},
    icons::icon,
    media::{Media, avatar},
    context_menu::Target,
    dm_picker::Purpose,
    profile,
    session::{MentionUser, OpenInbox, OpenMenu, OpenPicker, OpenUserMenu, PanelKind, Session},
    theme,
};

/// Width of the avatar column, which grouped messages leave empty so the
/// text lines up.
const GUTTER: f32 = 40.;

mod commands;
mod components;
mod menu;
use super::autocomplete::{self, Suggestion};

/// The message area for whichever channel is open: header, list, composer.
pub struct ChatView {
    session: Entity<Session>,
    media: Entity<Media>,
    backend: Backend,
    channel: Option<Snowflake>,
    profile_modal: Option<Snowflake>,
    list: ListState,
    rows: Vec<Row>,
    /// Channels whose first page of history has been requested this session.
    fetched: HashSet<Snowflake>,
    loading: bool,

    input: Entity<TextareaState>,
    /// Unsent text per channel, so switching away doesn't lose it.
    drafts: HashMap<Snowflake, String>,
    reply_to: Option<Snowflake>,
    /// Whether the reply notifies its author. Resets with each new reply.
    reply_ping: bool,
    /// Reactor lookups already sent, so hovering twice asks once.
    reactors_asked: std::collections::HashSet<(Snowflake, String, u32)>,
    /// Width of the column laid over the right edge, which the messages
    /// and the composer stay clear of.
    aside_width: f32,
    editing: Option<Snowflake>,
    /// Files chosen for the next message.
    files: Vec<PathBuf>,
    /// The message whose delete button was pressed once already.
    confirm_delete: Option<Snowflake>,
    /// Spoilers the reader has clicked open, as (message, index in message).
    revealed: HashSet<(Snowflake, usize)>,
    /// The message whose quick-reaction row is open.
    picker_for: Option<Snowflake>,
    /// A message briefly marked after jumping to it.
    highlight: Option<Snowflake>,
    /// People mentioned on screen whose names we do not have yet. Filled while
    /// drawing, so it sits behind a cell.
    unknown_users: std::cell::RefCell<HashSet<Snowflake>>,
    /// The option form of the slash command being filled in.
    command_form: Option<commands::CommandForm>,
    /// What the word being typed could become, and which one is highlighted.
    suggestions: Vec<Suggestion>,
    suggest_start: usize,
    suggest_selected: usize,
    /// The emoji picker, and what a pick from it is for.
    emoji: Entity<EmojiPanel>,
    emoji_for: Option<EmojiTarget>,
    /// The drop-down menu of a bot message that is open, as (message, custom id).
    open_menu: Option<(Snowflake, String)>,
    /// What was last chosen in each menu, so it shows the choice.
    picked: HashMap<(Snowflake, String), String>,

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

        let emoji = {
            let (session, media, backend) = (session.clone(), media.clone(), backend.clone());
            cx.new(|cx| EmojiPanel::new(session, media, backend, window, cx))
        };

        let subscriptions = vec![
            cx.subscribe_in(&emoji, window, |this, _, picked: &Picked, window, cx| {
                this.on_emoji(&picked.0, window, cx);
            }),
            cx.subscribe(&emoji, |this, _, picked: &PickedGif, cx| {
                this.send_gif(&picked.0, cx);
            }),
            cx.subscribe(&emoji, |this, _, picked: &PickedSticker, cx| {
                this.send_sticker(picked.0.id, cx);
            }),
            cx.subscribe(&emoji, |this, _, _: &ClosePicker, cx| {
                this.emoji_for = None;
                cx.notify();
            }),
            cx.subscribe(&session, |this, _, changes: &Changes, cx| {
                this.on_changes(changes, cx)
            }),
            cx.subscribe_in(&session, window, |this, _, mention: &MentionUser, window, cx| {
                this.insert_mention(mention.0, window, cx);
            }),
            cx.observe(&session, |_, _, cx| cx.notify()),
            cx.observe(&media, |_, _, cx| cx.notify()),
            cx.subscribe_in(&input, window, |this, _, event: &InputEvent, window, cx| match event {
                // With suggestions showing, Enter picks one instead of sending.
                InputEvent::PressEnter { shift: false, .. } if !this.suggestions.is_empty() => {
                    this.apply_suggestion(window, cx);
                }
                InputEvent::PressEnter { shift: false, .. } => this.submit(window, cx),
                InputEvent::Change => {
                    this.note_typing(window, cx);
                    this.refresh_suggestions(cx);
                }
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
            profile_modal: None,
            list,
            rows: Vec::new(),
            fetched: HashSet::new(),
            loading: false,
            input,
            drafts: HashMap::new(),
            reply_to: None,
            reply_ping: true,
            reactors_asked: Default::default(),
            aside_width: 0.,
            editing: None,
            files: Vec::new(),
            confirm_delete: None,
            revealed: HashSet::new(),
            picker_for: None,
            open_menu: None,
            highlight: None,
            unknown_users: std::cell::RefCell::new(HashSet::new()),
            command_form: None,
            suggestions: Vec::new(),
            suggest_start: 0,
            suggest_selected: 0,
            emoji,
            emoji_for: None,
            picked: HashMap::new(),
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
        self.files.clear();

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

    /// Asks the window to show `user`'s card, as seen from this channel's server.
    /// Puts `<@id> ` at the cursor, for the Mention entry of a person's menu.
    fn insert_mention(&mut self, user: Snowflake, window: &mut Window, cx: &mut Context<Self>) {
        if self.channel.is_none() || !self.can_send(cx) {
            return;
        }
        self.input.update(cx, |input, cx| {
            input.insert(format!("<@{user}> "), window, cx);
            input.focus(window, cx);
        });
    }

    fn user_menu_at(&mut self, user: Snowflake, event: &MouseDownEvent, cx: &mut Context<Self>) {
        cx.stop_propagation();
        let guild = self.guild(cx);
        let can_mention = self.can_send(cx);
        self.session.update(cx, |_, cx| {
            cx.emit(OpenUserMenu { user, guild, at: event.position, dm: None, can_mention });
        });
    }

    fn open_profile(&mut self, user: Snowflake, cx: &mut Context<Self>) {
        let guild = self
            .channel
            .and_then(|c| self.session.read(cx).store.channel(c))
            .and_then(|c| c.guild_id);
        self.session
            .update(cx, |session, cx| session.show_profile(user, guild, cx));
    }

    /// Goes to a message: straight there when it is loaded, otherwise after
    /// loading the history around it.
    pub fn focus_message(&mut self, message: Snowflake, cx: &mut Context<Self>) {
        let Some(channel) = self.channel else { return };
        if self.message(message, cx).is_some() {
            self.scroll_to_message(message, cx);
        } else {
            self.backend.send(Command::LoadAround { channel, message });
        }
    }

    /// Scrolls to a message the session was asked to show, once it is loaded.
    fn apply_jump_target(&mut self, cx: &mut Context<Self>) {
        let target = self.session.read(cx).jump_target;
        let (Some((channel, message)), Some(current)) = (target, self.channel) else { return };
        if channel == current && self.message(message, cx).is_some() {
            self.session.update(cx, |s, _| s.jump_target = None);
            self.scroll_to_message(message, cx);
        }
    }

    fn scroll_to_message(&mut self, message: Snowflake, cx: &mut Context<Self>) {
        let Some(ix) = self
            .rows
            .iter()
            .position(|row| matches!(row, Row::Message { id, .. } if *id == message))
        else {
            return;
        };
        self.list.scroll_to_reveal_item(ix);
        // Mark it for a moment so the eye finds it.
        self.highlight = Some(message);
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_secs(3)).await;
            this.update(cx, |this, cx| {
                if this.highlight == Some(message) {
                    this.highlight = None;
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    /// Leaves a jumped-to stretch of history and loads the newest messages.
    fn return_to_present(&mut self, cx: &mut Context<Self>) {
        let Some(channel) = self.channel else { return };
        self.session.update(cx, |s, cx| {
            let changes = s.store.return_to_present(channel);
            cx.emit(changes);
        });
        self.fetched.insert(channel);
        self.request(channel, None, cx);
    }

    /// The bar shown while looking at older history, with a way back.
    fn render_present_bar(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let channel = self.channel?;
        let detached = self
            .session
            .read(cx)
            .store
            .timeline(channel)
            .is_some_and(|t| t.is_detached());
        detached.then(|| {
            div()
                .id("jump-present")
                .mb_1()
                .px_3()
                .h(px(32.))
                .flex()
                .items_center()
                .justify_between()
                .rounded(px(theme::RADIUS))
                .bg(rgb(theme::ACCENT))
                .text_color(rgb(theme::TEXT_BRIGHT))
                .cursor_pointer()
                .hover(|s| s.opacity(0.9))
                .on_click(cx.listener(|this, _, _, cx| this.return_to_present(cx)))
                .child("You are viewing older messages")
                .child(div().font_weight(FontWeight::SEMIBOLD).child("Jump to present"))
                .into_any_element()
        })
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

    /// Rebuilds the suggestion list for the word at the end of the text.
    /// Only offered when the cursor is at the end, because accepting one
    /// rewrites the whole text.
    fn refresh_suggestions(&mut self, cx: &mut Context<Self>) {
        let (text, at_end) = {
            let input = self.input.read(cx);
            let text = input.value().to_string();
            let at_end = input.cursor() >= text.len();
            (text, at_end)
        };
        self.suggestions.clear();
        self.suggest_selected = 0;
        if let (true, Some(channel), Some(trigger)) = (at_end, self.channel, autocomplete::trigger(&text)) {
            let guild = self.guild(cx);
            self.suggestions = if trigger.kind == autocomplete::Kind::Command {
                self.command_suggestions(&trigger.query, cx)
            } else {
                autocomplete::suggest(&self.session.read(cx).store, guild, channel, &trigger)
            };
            self.suggest_start = trigger.start;
        }
        cx.notify();
    }

    fn apply_suggestion(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(pick) = self.suggestions.get(self.suggest_selected).cloned() else { return };
        // A command opens its option form instead of editing the text.
        if pick.command.is_some() {
            self.open_command_form(&pick, window, cx);
            return;
        }
        let mut text = self.input.read(cx).value().to_string();
        text.truncate(self.suggest_start.min(text.len()));
        text.push_str(&pick.insert);
        text.push(' ');
        self.suggestions.clear();
        self.input.update(cx, |input, cx| input.set_value(text, window, cx));
        cx.notify();
    }

    /// Arrow keys, Tab and Escape while suggestions are showing. Returns
    /// whether the key was used.
    fn suggestion_key(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.suggestions.is_empty() {
            return false;
        }
        let last = self.suggestions.len() - 1;
        match key {
            "down" => self.suggest_selected = (self.suggest_selected + 1).min(last),
            "up" => self.suggest_selected = self.suggest_selected.saturating_sub(1),
            "tab" => self.apply_suggestion(window, cx),
            "escape" => self.suggestions.clear(),
            _ => return false,
        }
        cx.notify();
        true
    }

    /// The list above the composer.
    fn render_suggestions(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.suggestions.is_empty() {
            return None;
        }
        let mut list = div()
            .mb_1()
            .p_1()
            .flex()
            .flex_col()
            .rounded(px(8.))
            .border_1()
            .border_color(rgb(theme::BORDER))
            .bg(rgb(theme::BG_PANEL));
        for (i, item) in self.suggestions.iter().enumerate() {
            let selected = i == self.suggest_selected;
            list = list.child(
                div()
                    .id(("suggestion", i as u64))
                    .px_2()
                    .h(px(30.))
                    .flex()
                    .items_center()
                    .gap_2()
                    .rounded(px(4.))
                    .cursor_pointer()
                    .bg(rgb(if selected { theme::BG_ACTIVE } else { theme::BG_PANEL }))
                    .hover(|s| s.bg(rgb(theme::BG_HOVER)))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.suggest_selected = i;
                        this.apply_suggestion(window, cx);
                    }))
                    .child(div().text_color(rgb(theme::TEXT_BRIGHT)).child(item.label.clone()))
                    .child(div().text_size(px(12.)).text_color(rgb(theme::TEXT_MUTED)).child(item.detail.clone())),
            );
        }
        Some(list.into_any_element())
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
            self.apply_jump_target(cx);
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
        let mut new = self
            .channel
            .and_then(|c| self.session.read(cx).store.timeline(c))
            .map(|t| build_rows(t.messages(), offset, self.unread_after))
            .unwrap_or_default();
        if self.shows_intro(cx) {
            new.insert(0, Row::Intro);
        }

        match diff(&self.rows, &new) {
            Some((range, count)) => self.list.splice(range, count),
            // Same rows, but a message may have been edited or reacted to,
            // which changes its height.
            None => self.list.remeasure(),
        }
        self.rows = new;
    }

    /// Whether the top of the list should say what conversation this is: only
    /// once nothing older can be fetched, and only for chats and channels
    /// that have such a beginning.
    fn shows_intro(&self, cx: &App) -> bool {
        let Some(channel) = self.channel else { return false };
        let session = self.session.read(cx);
        let fetched = session.store.timeline(channel).is_some_and(|t| !t.has_more_before());
        let kind = session.store.channel(channel).map(|c| c.kind);
        fetched
            && !self.loading
            && !session.history_errors.contains_key(&channel)
            && matches!(
                kind,
                Some(ChannelType::Dm | ChannelType::GroupDm | ChannelType::GuildText | ChannelType::GuildAnnouncement)
            )
    }

    fn render_intro(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Some(channel_id) = self.channel else { return div().into_any_element() };
        let (kind, name, topic, person) = {
            let store = &self.session.read(cx).store;
            let Some(channel) = store.channel(channel_id) else { return div().into_any_element() };
            let person = channel.recipients.first().map(|u| {
                (cdn::user_avatar(u.id, u.avatar.as_deref(), 160), u.display_name().to_owned(), tag(u))
            });
            let name = if matches!(channel.kind, ChannelType::Dm | ChannelType::GroupDm) {
                store.dm_name(channel)
            } else {
                channel.name.clone().unwrap_or_default()
            };
            (channel.kind, name, channel.topic.clone(), person)
        };
        match (kind, person) {
            (ChannelType::Dm, Some((url, display, username))) => intro::direct(&self.media, &url, &display, &username, cx),
            (ChannelType::GroupDm | ChannelType::Dm, _) => intro::group(&name),
            _ => intro::channel(&name, topic.as_deref()),
        }
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

    fn open_emoji(&mut self, target: EmojiTarget, window: &mut Window, cx: &mut Context<Self>) {
        if self.emoji_for == Some(target) {
            self.emoji_for = None;
        } else {
            let guild = self.guild(cx);
            self.emoji_for = Some(target);
            self.emoji.update(cx, |panel, cx| panel.show(guild, window, cx));
        }
        cx.notify();
    }

    fn open_gifs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let guild = self.guild(cx);
        self.emoji_for = Some(EmojiTarget::Composer);
        self.emoji.update(cx, |panel, cx| panel.show_gifs(guild, window, cx));
        cx.notify();
    }

    fn open_stickers(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let guild = self.guild(cx);
        self.emoji_for = Some(EmojiTarget::Composer);
        self.emoji.update(cx, |panel, cx| panel.show_stickers(guild, window, cx));
        cx.notify();
    }

    /// Sends a sticker on its own, answering the reply being written if any.
    fn send_sticker(&mut self, sticker: Snowflake, cx: &mut Context<Self>) {
        self.emoji_for = None;
        if let (Some(channel), true) = (self.channel, self.can_send(cx)) {
            let guild = self.guild(cx);
            self.backend.send(Command::SendMessage {
                channel,
                guild,
                content: String::new(),
                reply_to: self.reply_to.take(),
                ping_reply: self.reply_ping,
                files: Vec::new(),
                stickers: vec![sticker],
            });
        }
        cx.notify();
    }

    /// Posts a picked GIF: its address as the message, which Discord turns
    /// into an inline animation.
    fn send_gif(&mut self, gif: &oxicord_model::Gif, cx: &mut Context<Self>) {
        self.emoji_for = None;
        if let (Some(channel), true) = (self.channel, self.can_send(cx)) {
            let guild = self.guild(cx);
            self.backend.send(Command::SendMessage {
                channel,
                guild,
                content: gif.url.clone(),
                reply_to: self.reply_to.take(),
                ping_reply: self.reply_ping,
                files: Vec::new(),
                stickers: Vec::new(),
            });
        }
        cx.notify();
    }

    /// An emoji was picked: into the composer, or as a reaction.
    fn on_emoji(&mut self, emoji: &Emoji, window: &mut Window, cx: &mut Context<Self>) {
        match self.emoji_for.take() {
            Some(EmojiTarget::Composer) => {
                let text = match (emoji.id, emoji.name.as_deref()) {
                    (Some(id), Some(name)) => {
                        format!("<{}:{name}:{id}> ", if emoji.animated { "a" } else { "" })
                    }
                    (_, Some(glyph)) => glyph.to_owned(),
                    _ => String::new(),
                };
                self.input.update(cx, |input, cx| {
                    input.insert(text, window, cx);
                    input.focus(window, cx);
                });
            }
            Some(EmojiTarget::Reaction(message)) => self.react(message, emoji.clone(), true, cx),
            None => {}
        }
        cx.notify();
    }

    /// Whether we may post here. Threads have their own permission.
    fn can_send(&self, cx: &App) -> bool {
        let in_thread = self
            .channel
            .and_then(|c| self.session.read(cx).store.channel(c))
            .is_some_and(|c| c.kind.is_thread());
        let needed = if in_thread {
            Permissions::SEND_MESSAGES_IN_THREADS
        } else {
            Permissions::SEND_MESSAGES
        };
        self.permissions(cx).contains(needed)
    }

    fn guild(&self, cx: &App) -> Option<Snowflake> {
        let store = &self.session.read(cx).store;
        self.channel.and_then(|c| store.channel(c)).and_then(|c| c.guild_id)
    }

    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(channel) = self.channel else { return };
        if !self.can_send(cx) {
            return;
        }
        let text = self.input.read(cx).value().trim().to_owned();
        let sending_files = self.editing.is_none() && !self.files.is_empty();
        if text.is_empty() && !sending_files {
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
                    ping_reply: self.reply_ping,
                    files: std::mem::take(&mut self.files),
                    stickers: Vec::new(),
                });
            }
        }
        self.drafts.remove(&channel);
        self.input.update(cx, |input, cx| input.set_value("", window, cx));
        cx.notify();
    }

    /// Discord allows ten attachments on one message.
    const MAX_FILES: usize = 10;

    fn add_files(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        for path in paths {
            if self.files.len() < Self::MAX_FILES && !self.files.contains(&path) {
                self.files.push(path);
            }
        }
        cx.notify();
    }

    /// For the keyboard shortcuts. A picker toggles, like its button.
    pub fn shortcut(&mut self, action: Shortcut, window: &mut Window, cx: &mut Context<Self>) {
        if self.channel.is_none() || !self.can_send(cx) {
            return;
        }
        match action {
            Shortcut::Emoji | Shortcut::Gifs | Shortcut::Stickers if self.emoji_for.is_some() => {
                self.emoji_for = None
            }
            Shortcut::Emoji => self.open_emoji(EmojiTarget::Composer, window, cx),
            Shortcut::Gifs => self.open_gifs(window, cx),
            Shortcut::Stickers => self.open_stickers(window, cx),
            Shortcut::Upload => self.pick_files(cx),
        }
        cx.notify();
    }

    fn pick_files(&mut self, cx: &mut Context<Self>) {
        let chosen = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: None,
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = chosen.await {
                this.update(cx, |this, cx| this.add_files(paths, cx)).ok();
            }
        })
        .detach();
    }

    /// Reserves room on the right for a column the parent lays over the chat.
    pub fn set_aside_width(&mut self, width: f32, cx: &mut Context<Self>) {
        if (self.aside_width - width).abs() > f32::EPSILON {
            self.aside_width = width;
            self.list.remeasure();
            cx.notify();
        }
    }

    fn start_reply(&mut self, message: Snowflake, window: &mut Window, cx: &mut Context<Self>) {
        self.reply_to = Some(message);
        self.reply_ping = true;
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

    fn react(&mut self, message: Snowflake, emoji: Emoji, add: bool, cx: &mut Context<Self>) {
        if let Some(channel) = self.channel {
            self.backend.send(Command::React {
                channel,
                message,
                emoji,
                add,
            });
        }
        self.picker_for = None;
        cx.notify();
    }

    /// The pills under a message. Clicking one adds or removes our own
    /// reaction with that emoji.
    fn render_reactions(
        &mut self,
        message: &Message,
        can_react: bool,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if message.reactions.is_empty() {
            return None;
        }

        let mut row = div().flex().flex_wrap().gap_1().mt_1();
        for (ix, reaction) in message.reactions.iter().enumerate() {
            let (id, emoji, mine) = (message.id, reaction.emoji.clone(), reaction.me);

            let glyph = match reaction.emoji.id {
                Some(emoji_id) => {
                    let url = cdn::emoji(emoji_id, 32);
                    match self.media.update(cx, |media, _| media.get(&url)) {
                        Some(image) => gpui_kit::img(gpui_kit::ImageSource::Image(image))
                            .size(px(18.))
                            .into_any_element(),
                        None => div().size(px(18.)).into_any_element(),
                    }
                }
                None => div()
                    .text_size(px(16.))
                    .child(reaction.emoji.name.clone().unwrap_or_default())
                    .into_any_element(),
            };

            let mut pill = div()
                .id(SharedString::from(format!("reaction-{}-{ix}", message.id)))
                .flex()
                .items_center()
                .gap(px(6.))
                .h(px(30.))
                .px(px(7.))
                .rounded(px(8.))
                .border_1()
                .border_color(rgb(if mine { theme::ACCENT } else { theme::BG_HOVER }))
                .bg(rgb(if mine { 0x15183a } else { theme::BG_HOVER }))
                .text_size(px(13.))
                .text_color(rgb(if mine { 0x9aadff } else { theme::TEXT_MUTED }))
                .hover(|s| s.bg(rgb(if mine { 0x15183a } else { theme::BORDER })).border_color(rgb(if mine { theme::ACCENT } else { theme::BORDER_MUTED })))
                .child(glyph)
                .child(reaction.count.to_string());

            let key = (message.id, reaction.emoji.key(), reaction.count);
            let channel = message.channel_id;
            pill = pill
                .on_hover(cx.listener({
                    let (key, emoji) = (key.clone(), reaction.emoji.clone());
                    move |this, hovered: &bool, _, cx| {
                        if *hovered {
                            this.request_reactors(channel, &key, &emoji, cx);
                        }
                    }
                }))
                .tooltip({
                    let session = self.session.clone();
                    let name = reaction.emoji.name.clone().unwrap_or_default();
                    // Unicode reactions are named by the character, the tooltip wants :shortcode:.
                    let name = match reaction.emoji.id {
                        None => emojis::get(&name).and_then(|e| e.shortcode()).map_or(name, str::to_owned),
                        Some(_) => name,
                    };
                    move |_, cx| {
                        let names = session.read(cx).reactors.get(&key).cloned().unwrap_or_default();
                        let text = reactors_line(&names, key.2, &name);
                        cx.new(|_| Tip(text)).into()
                    }
                });
            if can_react {
                pill = pill.cursor_pointer().on_click(cx.listener(move |this, _, _, cx| {
                    this.react(id, emoji.clone(), !mine, cx);
                }));
            }
            row = row.child(pill);
        }
        Some(row.into_any_element())
    }

    /// Asks who reacted, once per message, emoji and count.
    fn request_reactors(&mut self, channel: Snowflake, key: &(Snowflake, String, u32), emoji: &Emoji, cx: &mut Context<Self>) {
        if self.reactors_asked.insert(key.clone()) {
            self.backend.send(Command::LoadReactors { channel, message: key.0, emoji: emoji.clone(), count: key.2 });
        }
        let _ = cx;
    }

    /// A row of common emoji to react with.
    fn render_picker(&mut self, message: Snowflake, cx: &mut Context<Self>) -> AnyElement {
        const QUICK: [&str; 10] = ["👍", "👎", "❤️", "😂", "😮", "😢", "🎉", "🔥", "👀", "🙏"];

        let mut row = div()
            .flex()
            .flex_wrap()
            .gap_1()
            .mt_1()
            .p_1()
            .rounded(px(8.))
            .border_1()
            .border_color(rgb(theme::BORDER))
            .bg(rgb(theme::BG_SIDEBAR))
            .w(px(340.));

        row = row.child(
            div()
                .id(SharedString::from(format!("quick-more-{message}")))
                .size(px(30.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(theme::RADIUS))
                .cursor_pointer()
                .hover(|s| s.bg(rgb(theme::BG_HOVER)))
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.picker_for = None;
                    this.open_emoji(EmojiTarget::Reaction(message), window, cx);
                }))
                .child(icon("face-slightly-smiling-plus", 18., theme::TEXT_MUTED)),
        );
        for (ix, glyph) in QUICK.into_iter().enumerate() {
            let emoji = Emoji {
                name: Some(glyph.to_owned()),
                ..Emoji::default()
            };
            row = row.child(
                div()
                    .id(SharedString::from(format!("quick-{message}-{ix}")))
                    .size(px(30.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(theme::RADIUS))
                    .cursor_pointer()
                    .text_size(px(18.))
                    .hover(|s| s.bg(rgb(theme::BG_HOVER)))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.react(message, emoji.clone(), true, cx);
                    }))
                    .child(glyph),
            );
        }
        row.into_any_element()
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
        let edited = message.edited_timestamp.is_some();
        self.render_text(message, &message.content, 0, edited, window, cx)
    }

    /// Draws `text` as the markdown of `message`. `salt` keeps the element ids
    /// of several texts in one message apart.
    fn render_text(
        &mut self,
        message: &Message,
        text: &str,
        salt: u64,
        edited: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let blocks = oxicord_markdown::parse(text);
        let prefs = self.view_prefs(cx);
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
            hour12: prefs.hour12,
            reveal_spoilers: prefs.reveal_spoilers,
            edited,
            key: id.0.wrapping_add(salt.wrapping_mul(0x9E37_79B9)),
            on_spoiler,
        };
        markdown::render(&blocks, &options, window, cx)
    }

    /// The reader's preferences that change how a message is drawn.
    fn view_prefs(&self, cx: &App) -> ViewPrefs {
        let account = &self.session.read(cx).account;
        let spoilers = account.pref(Key::RenderSpoilers);
        let reveal_spoilers = match spoilers.as_text() {
            "ALWAYS" => true,
            "IF_MODERATOR" => self.permissions(cx).contains(Permissions::MANAGE_MESSAGES),
            _ => false,
        };
        ViewPrefs {
            hour12: account.pref(Key::HourCycle).as_number() == 1,
            embeds: account.pref(Key::RenderEmbeds).as_bool(),
            embed_media: account.pref(Key::InlineEmbedMedia).as_bool(),
            attachment_media: account.pref(Key::InlineAttachmentMedia).as_bool(),
            reactions: account.pref(Key::RenderReactions).as_bool(),
            reveal_spoilers,
        }
    }

    /// Looks up the names a message's mentions refer to.
    fn names_for(&self, message: &Message, blocks: &[Block], cx: &App) -> Names {
        self.resolve_names(markdown::mention_ids(blocks), message, cx)
    }

    /// Names for the mentions inside a message's embeds, which hold plain
    /// text rather than parsed markdown.
    pub(super) fn embed_names(&self, message: &Message, cx: &App) -> Names {
        let mut text = String::new();
        for embed in &message.embeds {
            let parts = [
                embed.title.as_deref(),
                embed.description.as_deref(),
                embed.footer.as_ref().map(|f| f.text.as_str()),
                embed.author.as_ref().map(|a| a.name.as_str()),
            ];
            for part in parts.into_iter().flatten() {
                text.push_str(part);
                text.push(' ');
            }
            for field in &embed.fields {
                text.push_str(&field.name);
                text.push(' ');
                text.push_str(&field.value);
                text.push(' ');
            }
        }
        self.resolve_names(markdown::text_mention_ids(&text), message, cx)
    }

    fn resolve_names(&self, ids: markdown::MentionIds, message: &Message, cx: &App) -> Names {
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
            match name {
                Some(name) => {
                    names.users.insert(id, name);
                }
                // Someone we haven't met: ask for them once drawing is done.
                None => {
                    self.unknown_users.borrow_mut().insert(id);
                }
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

    /// Looks up the people drawing found mentioned but couldn't name.
    fn fetch_unknown_users(&mut self, cx: &mut Context<Self>) {
        let ids: Vec<Snowflake> = self.unknown_users.borrow_mut().drain().collect();
        if ids.is_empty() {
            return;
        }
        let backend = self.backend.clone();
        let guild = self.guild(cx);
        self.session.update(cx, |session, _| session.request_users(&backend, guild, ids));
    }

    fn render_row(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        match self.rows.get(ix).cloned() {
            Some(Row::Intro) => self.render_intro(cx),
            Some(Row::Divider(day)) => divider(day).into_any_element(),
            Some(Row::NewMessages) => new_messages_line().into_any_element(),
            Some(Row::Message { id, grouped }) => match self.message(id, cx) {
                Some(message) => {
                    let element = self.render_message(message, grouped, window, cx);
                    self.fetch_unknown_users(cx);
                    element
                }
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
        let can_reply = self.can_send(cx);
        let can_delete = is_mine || perms.contains(Permissions::MANAGE_MESSAGES);
        let can_react = perms.contains(Permissions::ADD_REACTIONS);
        let can_delete_others = perms.contains(Permissions::MANAGE_MESSAGES);
        let in_dm = message.guild_id.is_none() && self.guild(cx).is_none();
        let can_start_thread = !in_dm
            && perms.contains(Permissions::CREATE_PUBLIC_THREADS)
            && self
                .channel
                .and_then(|c| self.session.read(cx).store.channel(c))
                .is_some_and(|c| !c.kind.is_thread());

        let prefs = self.view_prefs(cx);
        let author = message.author.display_name().to_owned();
        let url = cdn::user_avatar(message.author.id, message.author.avatar.as_deref(), 64);

        let left: AnyElement = if grouped {
            // The time of a continued message appears in the gutter when the
            // row is hovered, ending where the official one does.
            div()
                .w(px(GUTTER))
                .flex_none()
                .relative()
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .right(px(0.))
                        .w(px(56.))
                        .h(px(22.))
                        .flex()
                        .items_center()
                        .justify_end()
                        .text_size(px(11.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(rgb(theme::TEXT_FAINT))
                        .invisible()
                        .group_hover("message", |s| s.visible())
                        .child(format::clock_time(message.timestamp, offset, prefs.hour12)),
                )
                .into_any_element()
        } else {
            let user = message.author.id;
            div()
                .id(("avatar", id.0))
                .on_mouse_down(MouseButton::Right, cx.listener(move |this, event: &MouseDownEvent, _, cx| this.user_menu_at(user, event, cx)))
                .cursor_pointer()
                .on_click(cx.listener(move |this, _, _, cx| this.open_profile(user, cx)))
                .child(avatar(&self.media, &url, GUTTER, &author, cx))
                .into_any_element()
        };

        let mut body = div().flex_1().min_w_0().flex().flex_col().line_height(px(22.));
        if !grouped {
            body = body.child(
                div()
                    .flex()
                    .items_baseline()
                    .gap(px(4.))
                    .child(
                        div()
                            .id(("author", id.0))
                            .on_mouse_down(MouseButton::Right, cx.listener({
                                let user = message.author.id;
                                move |this, event: &MouseDownEvent, _, cx| this.user_menu_at(user, event, cx)
                            }))
                            .cursor_pointer()
                            .hover(|s| s.underline())
                            .on_click(cx.listener({
                                let user = message.author.id;
                                move |this, _, _, cx| this.open_profile(user, cx)
                            }))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgb(theme::TEXT_BRIGHT))
                            .child(author),
                    )
                    .children(message.author.bot.then(crate::member_list::app_badge))
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(rgb(theme::CHAT_TEXT_MUTED))
                            .child(format::message_time(message.timestamp, Utc::now(), offset, prefs.hour12)),
                    ),
            );
        }
        let replied_to = message.message_reference.as_ref().and_then(|r| r.message_id);
        let open_original = cx.listener(move |this, _: &ClickEvent, _, cx| {
            if let Some(original) = replied_to {
                this.focus_message(original, cx);
            }
        });
        if let Some(reply) = reply_line(&message, open_original) {
            body = body.child(reply);
        }
        body = body.child(self.render_content(&message, window, cx));
        if let Some(files) = render_attachments(&message, &self.media, prefs.attachment_media, cx) {
            body = body.child(files);
        }
        if prefs.embeds
            && let Some(embeds) =
                render_embeds(&message, &self.media, prefs.embed_media, &self.embed_names(&message, cx), window, cx)
        {
            body = body.child(embeds);
        }
        if prefs.reactions
            && let Some(reactions) = self.render_reactions(&message, can_react, cx)
        {
            body = body.child(reactions);
        }
        if let Some(forwarded) = self.render_forwards(&message, window, cx) {
            body = body.child(forwarded);
        }
        if let Some(stickers) = self.render_stickers(&message, cx) {
            body = body.child(stickers);
        }
        if let Some(poll) = self.render_poll(&message, cx) {
            body = body.child(poll);
        }
        if let Some(parts) = self.render_components(&message, window, cx) {
            body = body.child(parts);
        }
        if let Some(thread) = &message.thread {
            body = body.child(self.thread_chip(thread, cx));
        }
        if self.picker_for == Some(id) {
            body = body.child(self.render_picker(id, cx));
        }

        let confirming = self.confirm_delete == Some(id);
        let allowed = Allowed {
            reply: can_reply,
            react: can_react,
            edit: is_mine,
            delete: can_delete,
            pin: (can_delete_others || in_dm).then_some(message.pinned),
            thread: can_start_thread && message.thread.is_none(),
        };
        let bar = self.action_bar(id, allowed, confirming, cx);

        let row = div()
            .id(("message", id.0))
            .group("message")
            .relative()
            .w_full()
            .flex()
            .gap_4()
            .pl(px(16.))
            .pr(px(24.))
            .pt(px(if grouped { 2. } else { 17. }))
            .pb(px(2.))
            .hover(|s| s.bg(rgb(theme::BG_HOVER)))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, _, cx| this.right_click_message(id, event, allowed, cx)),
            )
            .child(left)
            .child(body)
            .child(bar);
        if self.highlight == Some(id) {
            return row.bg(rgb(theme::MESSAGE_HIGHLIGHT)).into_any_element();
        }
        row.into_any_element()
    }

    /// Reply, edit and delete, floating at the top right of a hovered
    /// message. While a delete is waiting for confirmation it stays visible.
    fn action_bar(
        &mut self,
        id: Snowflake,
        allowed: Allowed,
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
            .rounded(px(theme::RADIUS))
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

        if allowed.react {
            bar = bar.child(
                action_button(("react", id.0), "face-slightly-smiling-plus", theme::TEXT_MUTED).on_click(
                    cx.listener(move |this, _, _, cx| {
                        this.picker_for = (this.picker_for != Some(id)).then_some(id);
                        cx.notify();
                    }),
                ),
            );
        }
        if allowed.edit {
            bar = bar.child(
                action_button(("edit", id.0), "pencil", theme::TEXT_MUTED).on_click(
                    cx.listener(move |this, _, window, cx| this.start_edit(id, window, cx)),
                ),
            );
        }
        if allowed.reply {
            bar = bar.child(
                action_button(("reply", id.0), "reply", theme::TEXT_MUTED).on_click(
                    cx.listener(move |this, _, window, cx| this.start_reply(id, window, cx)),
                ),
            );
        }
        bar.child(
            action_button(("forward", id.0), "forward", theme::TEXT_MUTED)
                .on_click(cx.listener(move |this, _, _, cx| this.forward(id, cx))),
        )
        .child(
            action_button(("more", id.0), "ellipsis", theme::TEXT_MUTED).on_click(cx.listener(
                move |this, event: &ClickEvent, _, cx| {
                    let at = gpui_kit::point(event.position().x - px(200.), event.position().y + px(14.));
                    this.open_message_menu(id, at, allowed, cx);
                },
            )),
        )
    }

    fn render_header(&mut self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let has_members = self.guild(cx).is_some();
        let showing = self.session.read(cx).settings.show_members;
        let (glyph, name, topic) = self
            .channel
            .and_then(|id| {
                let store = &self.session.read(cx).store;
                let channel = store.channel(id)?;
                Some(if channel.kind.is_dm() {
                    ("at-sign", store.dm_name(channel), None)
                } else if channel.kind.is_thread() {
                    let parent = channel
                        .parent_id
                        .and_then(|p| store.channel(p))
                        .and_then(|p| p.name.clone())
                        .map(|n| format!("in #{n}"));
                    ("messages-square", channel.name.clone().unwrap_or_default(), parent)
                } else {
                    ("hash", channel.name.clone().unwrap_or_default(), channel.topic.clone())
                })
            })
            .unwrap_or(("hash", String::new(), None));
        // A one to one chat is headed by the person's picture and what they
        // are up to, not a channel glyph.
        let dm_person = self.channel.and_then(|id| {
            let store = &self.session.read(cx).store;
            let channel = store.channel(id).filter(|c| c.kind == ChannelType::Dm)?;
            let user = channel.recipients.first()?;
            Some((
                cdn::user_avatar(user.id, user.avatar.as_deref(), 64),
                crate::friends::status_line(store.activities(user.id)),
                tag(user),
            ))
        });
        // Threads only make sense inside a server.
        let has_threads = self.guild(cx).is_some();
        let in_dm = self.channel.and_then(|id| self.session.read(cx).store.channel(id)).is_some_and(|c| c.kind.is_dm());
        let thread_state = self.channel.and_then(|id| {
            let c = self.session.read(cx).store.channel(id)?.clone();
            c.kind.is_thread().then(|| (id, c.joined(), c.archived()))
        });

        div()
            .h(px(theme::CHANNEL_HEADER_HEIGHT))
            .flex_none()
            .flex()
            .items_center()
            .gap(px(8.))
            .pl(px(16.))
            .pr(px(8.))
            .border_b_1()
            .border_color(rgb(theme::BORDER))
            .child(match &dm_person {
                Some((url, _, _)) => avatar(&self.media, url, 20., &name, cx).into_any_element(),
                None => icon(glyph, 20., theme::CHANNELS_DEFAULT).into_any_element(),
            })
            .child(
                div()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(theme::HEADER_PRIMARY))
                    .child(name),
            )
            .children(dm_person.as_ref().and_then(|(_, line, _)| line.clone()).map(|line| {
                div().ml(px(8.)).text_size(px(14.)).text_color(rgb(theme::TEXT_MUTED)).child(line)
            }))
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
            .child(div().flex_1())
            .children(thread_state.map(|(thread, joined, archived)| {
                let label = if joined { "Leave" } else { "Join thread" };
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .children(archived.then(|| {
                        div()
                            .text_size(px(12.))
                            .text_color(rgb(theme::TEXT_MUTED))
                            .child("Archived")
                    }))
                    .child(
                        div()
                            .id("thread-membership")
                            .h(px(26.))
                            .px_3()
                            .flex()
                            .items_center()
                            .rounded(px(theme::RADIUS))
                            .cursor_pointer()
                            .bg(rgb(if joined { theme::BG_ACTIVE } else { theme::ACCENT }))
                            .text_size(px(13.))
                            .text_color(rgb(theme::TEXT_BRIGHT))
                            .hover(|s| s.opacity(0.85))
                            .on_click(cx.listener(move |this, _, _, cx| this.set_membership(thread, !joined, cx)))
                            .child(label),
                    )
            }))
            .children(has_threads.then(|| {
                toolbar_button("open-threads", "messages-square", theme::TEXT_MUTED).on_click(
                    cx.listener(|this, _, _, cx| {
                        this.session.update(cx, |s, cx| s.open_panel(PanelKind::Threads, cx));
                    }),
                )
            }))
            .children(has_threads.then(|| {
                toolbar_button("notification-settings", "bell", theme::TEXT_MUTED).on_click(cx.listener(
                    |this, event: &ClickEvent, _, cx| {
                        let Some(channel) = this.channel else { return };
                        let at = gpui_kit::point(event.position().x - px(190.), event.position().y + px(18.));
                        this.session.update(cx, |_, cx| {
                            cx.emit(OpenMenu { target: Target::Channel(channel), at });
                        });
                    },
                ))
            }))
            .child(
                toolbar_button("open-pins", "pin", theme::TEXT_MUTED).on_click(cx.listener(
                    |this, _, _, cx| this.session.update(cx, |s, cx| s.open_panel(PanelKind::Pins, cx)),
                )),
            )
            .children(in_dm.then(|| {
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        toolbar_button("start-voice-call", "phone", theme::TEXT_MUTED)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.session.update(cx, |s, cx| s.open_panel(PanelKind::VoiceDetails, cx));
                            })),
                    )
                    .child(
                        toolbar_button("start-video-call", "video", theme::TEXT_MUTED)
                            .on_click(cx.listener(|_, _, _, _| {})),
                    )
            }))
            .children(in_dm.then(|| {
                toolbar_button("add-to-dm", "user-plus", theme::TEXT_MUTED).on_click(cx.listener(
                    |this, event: &ClickEvent, _, cx| {
                        let Some(channel) = this.channel else { return };
                        let at = gpui_kit::point(event.position().x - px(300.), event.position().y + px(18.));
                        this.session.update(cx, |_, cx| {
                            cx.emit(OpenPicker { purpose: Purpose::AddTo(channel), at });
                        });
                    },
                ))
            }))
            .children(in_dm.then(|| {
                toolbar_button("show-profile", "circle-user-round", theme::TEXT_MUTED).on_click(cx.listener(
                    |this, _, _, cx| {
                        if let Some(user) = this
                            .channel
                            .and_then(|id| this.session.read(cx).store.channel(id))
                            .and_then(|c| c.recipients.first().map(|u| u.id))
                        {
                            this.open_profile(user, cx);
                        }
                    },
                ))
            }))
            .children(in_dm.then(|| {
                toolbar_button("view-full-profile", "user", theme::TEXT_MUTED).on_click(cx.listener(
                    |this, _, _, cx| {
                        if let Some(user) = this
                            .channel
                            .and_then(|id| this.session.read(cx).store.channel(id))
                            .and_then(|c| c.recipients.first().map(|u| u.id))
                        {
                            this.profile_modal = Some(user);
                            cx.notify();
                        }
                    },
                ))
            }))
            .children((has_members || in_dm).then(|| {
                let color = if showing { theme::TEXT_BRIGHT } else { theme::TEXT_MUTED };
                toolbar_button("toggle-members", if in_dm { "circle-user-round" } else { "users" }, color)
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_members(cx)))
            }))
            .child(
                div()
                    .id("open-search")
                    .w(px(if dm_person.is_some() { 244. } else { 144. }))
                    .h(px(32.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_between()
                    .pl(px(8.))
                    .pr(px(4.))
                    .rounded(px(theme::RADIUS))
                    .border_1()
                    .border_color(rgb(theme::BORDER))
                    .cursor_text()
                    .bg(rgb(theme::BG_CHAT))
                    .text_size(px(14.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(theme::TEXT_FAINT))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.session.update(cx, |s, cx| s.open_panel(PanelKind::Search, cx))
                    }))
                    .child(div().truncate().child(match &dm_person {
                        Some((_, _, handle)) => format!("Search {handle}"),
                        None => "Search".to_owned(),
                    }))
                    .child(div().size(px(24.)).flex().items_center().justify_center().child(icon("search", 16., theme::TEXT_MUTED))),
            )
            .child(
                toolbar_button("open-inbox", "inbox", theme::TEXT_MUTED).on_click(cx.listener(|this, _, _, cx| {
                    this.session.update(cx, |_, cx| cx.emit(OpenInbox));
                })),
            )
            .child(
                toolbar_button("open-help", "circle-question-mark", theme::TEXT_MUTED)
                    .on_click(|_, _, cx| cx.open_url("https://support.discord.com")),
            )
    }


    /// The small card under a message that started a thread.
    fn thread_chip(&mut self, thread: &oxicord_model::Channel, cx: &mut Context<Self>) -> AnyElement {
        let id = thread.id;
        let known = thread.clone();
        let count = thread.message_count.map_or(String::new(), |n| {
            format!("{n} message{}", if n == 1 { "" } else { "s" })
        });
        div()
            .id(("thread-chip", id.0))
            .mt_1()
            .max_w(px(420.))
            .px_3()
            .py(px(6.))
            .flex()
            .items_center()
            .gap_2()
            .rounded(px(theme::RADIUS))
            .border_1()
            .border_color(rgb(theme::BORDER))
            .bg(rgb(theme::BG_SIDEBAR))
            .cursor_pointer()
            .hover(|s| s.bg(rgb(theme::BG_HOVER)))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.session.update(cx, |s, cx| {
                    s.store.add_thread(known.clone(), false);
                    s.jump_to(id, cx);
                });
            }))
            .child(icon("messages-square", 14., theme::TEXT_MUTED))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_color(rgb(markdown::LINK_COLOR))
                    .child(thread.name.clone().unwrap_or_default()),
            )
            .child(div().text_size(px(12.)).text_color(rgb(theme::TEXT_MUTED)).child(count))
            .into_any_element()
    }

    fn copy_text(&mut self, message: Snowflake, cx: &mut Context<Self>) {
        if let Some(m) = self.message(message, cx) {
            cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(m.content));
        }
    }

    fn copy_link(&mut self, message: Snowflake, cx: &mut Context<Self>) {
        if let Some(channel) = self.channel {
            let link = crate::menus::message_link(self.guild(cx), channel, message);
            cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(link));
        }
    }

    /// Starts a thread on `message`, named after what it says.
    fn start_thread(&mut self, message: Snowflake, cx: &mut Context<Self>) {
        let (Some(channel), Some(original)) = (self.channel, self.message(message, cx)) else {
            return;
        };
        let name: String = original
            .content
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .chars()
            .take(60)
            .collect();
        let name = if name.is_empty() { "New thread".to_owned() } else { name };
        self.backend.send(Command::CreateThread { channel, message, name });
    }

    /// Joins or leaves a thread, shown at once and confirmed by the gateway.
    fn set_membership(&mut self, thread: Snowflake, join: bool, cx: &mut Context<Self>) {
        self.session.update(cx, |s, cx| {
            let changes = s.store.set_thread_joined(thread, join);
            cx.emit(changes);
            cx.notify();
        });
        self.backend.send(if join {
            Command::JoinThread(thread)
        } else {
            Command::LeaveThread(thread)
        });
    }

    fn set_pinned(&mut self, message: Snowflake, pinned: bool, cx: &mut Context<Self>) {
        let Some(channel) = self.channel else { return };
        let backend = self.backend.clone();
        self.session
            .update(cx, |s, cx| s.set_pinned(&backend, channel, message, pinned, cx));
    }

    fn toggle_members(&mut self, cx: &mut Context<Self>) {
        let backend = self.backend.clone();
        let showing = self.session.update(cx, |session, cx| {
            session.change_settings(&backend, |s| s.show_members = !s.show_members, cx);
            session.settings.show_members
        });
        if let (true, Some(guild), Some(channel)) = (showing, self.guild(cx), self.channel) {
            self.backend.send(Command::SubscribeMembers { guild, channel });
        }
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
        let can_send = self.can_send(cx);
        let typing = self.typing_line(cx);
        self.typing_shown = typing.is_some();

        let mut column = div().flex_none().flex().flex_col().px(px(8.)).pb(px(8.)).pt_1();

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
                    .rounded(px(theme::RADIUS))
                    .bg(rgb(0x120707))
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

        if let Some(bar) = self.render_present_bar(cx) {
            column = column.child(bar);
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

        if let Some(list) = self.render_suggestions(cx) {
            column = column.child(list);
        }
        if let Some(form) = self.render_command_form(cx) {
            column = column.child(form);
        }
        if let Some(bar) = self.compose_context_bar(cx) {
            column = column.child(bar);
        }
        if !self.files.is_empty() && self.editing.is_none() {
            column = column.child(self.render_file_chips(cx));
        }

        column
            .child(
                div()
                    .flex()
                    .items_end()
                    .gap_2()
                    .px_3()
                    .py(px(6.))
                    .rounded(px(theme::RADIUS))
                    .border_1()
                    .border_color(rgb(theme::BORDER_MUTED))
                    .bg(rgb(theme::BG_INPUT))
                    .on_drop(cx.listener(|this, dropped: &ExternalPaths, _, cx| {
                        this.add_files(dropped.paths().to_vec(), cx);
                    }))
                    .children((self.editing.is_none()).then(|| {
                        toolbar_button("attach", "circle-plus", theme::TEXT_MUTED)
                            .on_click(cx.listener(|this, _, _, cx| this.pick_files(cx)))
                    }))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            // Captured before the text field turns arrows into
                            // cursor moves, so the list can use them.
                            .capture_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                                if this.suggestion_key(event.keystroke.key.as_str(), window, cx) {
                                    cx.stop_propagation();
                                }
                            }))
                            .child(Textarea::new(&self.input).appearance(false).bordered(false)),
                    )
                    .child(
                        toolbar_button("gif", "images", theme::TEXT_MUTED).on_click(cx.listener(
                            |this, _, window, cx| this.open_gifs(window, cx),
                        )),
                    )
                    .child(
                        toolbar_button("sticker", "sticker", theme::TEXT_MUTED).on_click(cx.listener(
                            |this, _, window, cx| this.open_stickers(window, cx),
                        )),
                    )
                    .child(
                        toolbar_button("emoji", "face-slightly-smiling", theme::TEXT_MUTED).on_click(cx.listener(
                            |this, _, window, cx| this.open_emoji(EmojiTarget::Composer, window, cx),
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
    /// The files waiting to be sent, each removable.
    fn render_file_chips(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let mut row = div().flex().flex_wrap().gap_2().mb_2();
        for (index, path) in self.files.iter().enumerate() {
            let name = path
                .file_name()
                .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
            row = row.child(
                div()
                    .max_w(px(240.))
                    .h(px(30.))
                    .pl_3()
                    .pr_1()
                    .flex()
                    .items_center()
                    .gap_2()
                    .rounded(px(theme::RADIUS))
                    .bg(rgb(theme::BG_INPUT))
                    .text_size(px(13.))
                    .child(icon("file", 14., theme::TEXT_MUTED))
                    .child(div().flex_1().min_w_0().truncate().child(name))
                    .child(
                        action_button(("remove-file", index), "x", theme::TEXT_MUTED).on_click(
                            cx.listener(move |this, _, _, cx| {
                                if index < this.files.len() {
                                    this.files.remove(index);
                                }
                                cx.notify();
                            }),
                        ),
                    ),
            );
        }
        row.into_any_element()
    }

    fn compose_context_bar(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        // The name is only a heading for replies. Pinging is offered in
        // servers, and never for a reply to ourselves.
        let (label, ping_choice) = if self.editing.is_some() {
            (div().child("Editing message"), None)
        } else {
            let reply = self.reply_to?;
            let author = self.message(reply, cx).map(|m| m.author);
            let mine = author.as_ref().zip(self.session.read(cx).store.me()).is_some_and(|(a, me)| a.id == me.id);
            let name = author.map(|a| a.display_name().to_owned()).unwrap_or_else(|| "message".into());
            (
                div()
                    .flex()
                    .gap(px(4.))
                    .child("Replying to")
                    .child(div().font_weight(FontWeight::SEMIBOLD).text_color(rgb(theme::TEXT_BRIGHT)).child(name)),
                (self.guild(cx).is_some() && !mine).then_some(self.reply_ping),
            )
        };

        Some(
            div()
                .h(px(38.))
                .flex()
                .items_center()
                .gap(px(8.))
                .px(px(16.))
                .rounded_t(px(8.))
                .bg(rgb(theme::BG_PANEL))
                .text_size(px(14.))
                .text_color(rgb(theme::TEXT_MUTED))
                .child(label.flex_1())
                .children(ping_choice.map(|on| {
                    div()
                        .id("reply-ping")
                        .h(px(24.))
                        .px(px(6.))
                        .flex()
                        .items_center()
                        .gap(px(4.))
                        .rounded(px(4.))
                        .cursor_pointer()
                        .text_size(px(12.))
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(if on { theme::TEXT_BRIGHT } else { theme::TEXT_FAINT }))
                        .hover(|s| s.bg(rgb(theme::BG_HOVER)))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.reply_ping = !this.reply_ping;
                            cx.notify();
                        }))
                        .child("@")
                        .child(if on { "ON" } else { "OFF" })
                }))
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
        let picker = self.emoji_for.is_some().then(|| self.emoji.clone());
        let header = self.render_header(cx);
        let body = self.render_body(cx);
        let composer = self.render_composer(cx);

        div()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .flex_col()
            .relative()
            .bg(rgb(theme::BG_CHAT))
            // Escape closes what is open here first. Only when there is
            // nothing does it go on up, where it marks the channel read.
            .on_action(cx.listener(|this, _: &Escape, window, cx| {
                if this.emoji_for.is_some() {
                    this.emoji_for = None;
                    cx.notify();
                } else if this.reply_to.is_some() || this.editing.is_some() {
                    this.cancel_compose(window, cx);
                } else {
                    cx.propagate();
                }
            }))
            .child(header)
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .pr(px(self.aside_width))
                    .child(div().flex_1().min_h_0().child(body))
                    .child(composer),
            )
            .children(picker.map(|panel| {
                div().absolute().bottom(px(72.)).right(px(16. + self.aside_width)).child(panel)
            }))
            .children(self.profile_modal.map(|user| {
                let (name, username, pronouns, avatar_url, status, bio, mutual_friends, mutual_servers, badges) = {
                    let session = self.session.read(cx);
                    let store = &session.store;
                    let account = store.user(user);
                    let profile = session.profiles.get(&user);
                    (
                        account.map(|u| u.display_name().to_owned()).unwrap_or_default(),
                        account.map(|u| u.username.clone()).unwrap_or_default(),
                        profile.map(|p| p.pronouns.clone()).unwrap_or_default(),
                        account.map(|u| cdn::user_avatar(u.id, u.avatar.as_deref(), 128)).unwrap_or_default(),
                        account.and_then(|u| store.activities(u.id).first().map(|a| a.name.clone())).unwrap_or_default(),
                        profile.map(|p| p.bio.clone()).unwrap_or_default(),
                        profile.map(|p| p.mutual_friends).unwrap_or(0),
                        profile.map(|p| p.mutual_guilds.len()).unwrap_or(0),
                        profile.map(|p| p.badges.clone()).unwrap_or_default(),
                    )
                };
                let on_close = cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.profile_modal = None;
                    cx.notify();
                });
                profile::modal(
                    div()
                        .w(px(420.))
                        .h(px(400.))
                        .bg(rgb(theme::BG_PANEL))
                        .rounded(px(theme::RADIUS))
                        .p_4()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(
                            div()
                                .flex()
                                .justify_end()
                                .child(
                                    div()
                                        .id("close-profile-modal")
                                        .size(px(28.))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .rounded(px(4.))
                                        .cursor_pointer()
                                        .hover(|s| s.bg(rgb(theme::BG_HOVER)))
                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                            this.profile_modal = None;
                                            cx.notify();
                                        }))
                                        .child(icon("x", 16., theme::TEXT_MUTED)),
                                ),
                        )
                        .child(avatar(&self.media, &avatar_url, 64., &name, cx))
                        .child(div().font_weight(FontWeight::BOLD).child(name.clone()))
                        .children((!username.is_empty()).then(|| div().text_size(px(12.)).text_color(rgb(theme::TEXT_MUTED)).child(format!("@{}", username))))
                        .children((!pronouns.is_empty()).then(|| div().text_size(px(12.)).text_color(rgb(theme::TEXT_MUTED)).child(pronouns)))
                        .children((!status.is_empty()).then(|| div().text_color(rgb(theme::TEXT_MUTED)).child(status)))
                        .children((!bio.is_empty()).then(|| div().text_size(px(13.)).child(bio)))
                        .child(div().text_size(px(12.)).text_color(rgb(theme::TEXT_MUTED)).child(format!("{} Mutual Friends", mutual_friends)))
                        .child(div().text_size(px(12.)).text_color(rgb(theme::TEXT_MUTED)).child(format!("{} Mutual Servers", mutual_servers)))
                        .children((!badges.is_empty()).then(|| {
                            div()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(div().text_size(px(12.)).font_weight(FontWeight::SEMIBOLD).child("Badges"))
                                .children(badges.iter().map(|badge| {
                                    div()
                                        .text_size(px(12.))
                                        .text_color(rgb(theme::TEXT_MUTED))
                                        .child(if badge.description.is_empty() {
                                            badge.id.clone()
                                        } else {
                                            badge.description.clone()
                                        })
                                }))
                        }))
                        .into_any_element(),
                    Box::new(on_close),
                )
            }))
    }
}

/// Composer actions that have a keyboard shortcut.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shortcut {
    Emoji,
    Gifs,
    Stickers,
    Upload,
}

/// What the emoji picker is currently choosing for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EmojiTarget {
    Composer,
    Reaction(Snowflake),
}

/// Display preferences read from the account, resolved for one redraw.
struct ViewPrefs {
    hour12: bool,
    embeds: bool,
    embed_media: bool,
    attachment_media: bool,
    reactions: bool,
    reveal_spoilers: bool,
}

/// What the reader may do to one message.
#[derive(Clone, Copy)]
struct Allowed {
    reply: bool,
    react: bool,
    edit: bool,
    delete: bool,
    /// May pin or unpin, and whether the message is pinned now.
    pin: Option<bool>,
    /// May start a thread on this message.
    thread: bool,
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

/// A toolbar button in the header or composer: a 32px square with a 20px icon.
fn toolbar_button(
    id: impl Into<gpui_kit::ElementId>,
    glyph: &str,
    color: u32,
) -> gpui_kit::Stateful<gpui_kit::Div> {
    div()
        .id(id)
        .size(px(32.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(4.))
        .cursor_pointer()
        .hover(|s| s.bg(rgb(theme::BG_HOVER)))
        .child(icon(glyph, 20., color))
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
        .pl(px(16.)).pr(px(14.))
        .mt(px(24.))
        .mb(px(8.))
        .child(line())
        .child(
            div()
                .text_size(px(12.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(theme::TEXT_FAINT))
                .child(format::day_label(day)),
        )
        .child(line())
}

/// The quoted line above a reply: who it answers and what they said.
fn reply_line(
    message: &Message,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Option<AnyElement> {
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
            .id(("reply-line", message.id.0))
            .flex()
            .items_center()
            .gap_2()
            .mb_1()
            .text_size(px(13.))
            .text_color(rgb(theme::TEXT_MUTED))
            .cursor_pointer()
            .hover(|s| s.text_color(rgb(theme::TEXT)))
            .on_click(on_click)
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

/// The handle people go by: a plain username, or the old name#1234 form.
fn tag(user: &oxicord_model::User) -> String {
    match user.discriminator.as_deref() {
        Some(d) if d != "0" && !d.is_empty() => format!("{}#{d}", user.username),
        _ => user.username.clone(),
    }
}

/// "Ann, Bo and Cy reacted with :wave:", counting those we did not fetch.
fn reactors_line(names: &[String], count: u32, emoji: &str) -> String {
    let word = format!(":{emoji}:");
    let shown = names.len() as u32;
    match names {
        [] => format!("{count} reacted with {word}"),
        [only] if count <= 1 => format!("{only} reacted with {word}"),
        _ if count > shown => format!("{} and {} others reacted with {word}", names.join(", "), count - shown),
        [rest @ .., last] => format!("{} and {last} reacted with {word}", rest.join(", ")),
    }
}

/// A plain text tooltip in the menu colors.
struct Tip(String);

impl Render for Tip {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .px(px(12.))
            .py(px(8.))
            .rounded(px(theme::RADIUS))
            .border_1()
            .border_color(rgb(theme::BORDER_MUTED))
            .bg(rgb(theme::BG_MENU))
            .text_size(px(14.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(rgb(theme::TEXT_BRIGHT))
            .child(self.0.clone())
    }
}

#[cfg(test)]
mod reactor_tests {
    use super::reactors_line;

    #[test]
    fn the_tooltip_lists_names_and_counts_the_rest() {
        let n = |s: &[&str]| s.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        assert_eq!(reactors_line(&n(&["Ann"]), 1, "wave"), "Ann reacted with :wave:");
        assert_eq!(reactors_line(&n(&["Ann", "Bo"]), 2, "wave"), "Ann and Bo reacted with :wave:");
        assert_eq!(reactors_line(&n(&["Ann", "Bo", "Cy"]), 3, "wave"), "Ann, Bo and Cy reacted with :wave:");
        assert_eq!(reactors_line(&n(&["Ann", "Bo", "Cy"]), 9, "wave"), "Ann, Bo, Cy and 6 others reacted with :wave:");
        assert_eq!(reactors_line(&[], 4, "wave"), "4 reacted with :wave:");
    }
}
