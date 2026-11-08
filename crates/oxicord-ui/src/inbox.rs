//! The inbox popout in the top right: what mentioned us, and what is unread.

use chrono::Local;
use gpui_kit::{
    AnyElement, ClickEvent, Context, Entity, EventEmitter, FontWeight, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _, Subscription, Window, div, px, rgb,
};
use oxicord_media::cdn;
use oxicord_model::{ChannelType, Message, Snowflake};

use crate::{
    account::Load,
    backend::{Backend, Command},
    icons::icon,
    media::{Media, avatar},
    session::Session,
    theme,
};

/// Longest stretch of a message shown in the list.
const PREVIEW_CHARS: usize = 240;

pub struct CloseInbox;

impl EventEmitter<CloseInbox> for Inbox {}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Unreads,
    Mentions,
    ForYou,
    Bookmarks,
    Reminders,
    Scheduled,
}

/// A channel with something new in it.
struct Pending {
    channel: Snowflake,
    name: String,
    place: String,
    mentions: u32,
    newest: Option<Snowflake>,
}

pub struct Inbox {
    session: Entity<Session>,
    media: Entity<Media>,
    backend: Backend,
    tab: Tab,
    _subscriptions: Vec<Subscription>,
}

impl Inbox {
    pub fn new(session: Entity<Session>, media: Entity<Media>, backend: Backend, cx: &mut Context<Self>) -> Self {
        let subscriptions = vec![cx.observe(&session, |_, _, cx| cx.notify()), cx.observe(&media, |_, _, cx| cx.notify())];
        Self { session, media, backend, tab: Tab::Mentions, _subscriptions: subscriptions }
    }

    /// Opening always refreshes the mentions, they go stale quickly.
    pub fn opened(&mut self, cx: &mut Context<Self>) {
        let backend = self.backend.clone();
        self.session.update(cx, |s, cx| s.request_mentions(&backend, cx));
        self.session.update(cx, |s, cx| s.request_saved_messages(&backend, cx));
        self.session.update(cx, |s, cx| s.request_scheduled_messages(&backend, cx));
        self.session.update(cx, |s, cx| s.request_notification_center(&backend, cx));
        cx.notify();
    }

    fn pending(&self, cx: &gpui_kit::App) -> Vec<Pending> {
        let store = &self.session.read(cx).store;
        let mut out = Vec::new();
        for guild in store.guilds() {
            for group in store.channel_groups(guild.id) {
                for c in group.channels {
                    if matches!(c.kind, ChannelType::GuildVoice | ChannelType::GuildStageVoice) {
                        continue;
                    }
                    let unread = store.unread(c.id);
                    if unread.unread || unread.mentions > 0 {
                        out.push(Pending {
                            channel: c.id,
                            name: format!("#{}", c.name.clone().unwrap_or_default()),
                            place: guild.name.clone(),
                            mentions: unread.mentions,
                            newest: c.last_message_id,
                        });
                    }
                }
            }
        }
        for c in store.dm_channels() {
            let unread = store.unread(c.id);
            if unread.unread || unread.mentions > 0 {
                out.push(Pending {
                    channel: c.id,
                    name: store.dm_name(c),
                    place: "Direct Messages".into(),
                    mentions: unread.mentions,
                    newest: c.last_message_id,
                });
            }
        }
        // What pinged us first.
        out.sort_by_key(|p| std::cmp::Reverse(p.mentions));
        out
    }

    fn mark_all_read(&mut self, cx: &mut Context<Self>) {
        let acks: Vec<_> = self.pending(cx).into_iter().filter_map(|p| p.newest.map(|m| (p.channel, m))).collect();
        if !acks.is_empty() {
            self.backend.send(Command::AckBulk(acks));
        }
    }

    fn go(&mut self, channel: Snowflake, message: Option<Snowflake>, cx: &mut Context<Self>) {
        self.session.update(cx, |s, cx| match message {
            Some(message) => s.jump_to_message(channel, message, cx),
            None => s.jump_to(channel, cx),
        });
        cx.emit(CloseInbox);
    }

    fn tab_button(&self, tab: Tab, label: &'static str, cx: &mut Context<Self>) -> AnyElement {
        let on = self.tab == tab;
        div()
            .id(label)
            .h(px(28.))
            .px_3()
            .flex()
            .items_center()
            .rounded(px(4.))
            .cursor_pointer()
            .text_size(px(13.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(rgb(if on { theme::TEXT_BRIGHT } else { theme::TEXT_MUTED }))
            .bg(rgb(if on { theme::BG_ACTIVE } else { theme::BG_SIDEBAR }))
            .hover(|s| s.bg(rgb(theme::BG_HOVER)))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.tab = tab;
                cx.notify();
            }))
            .child(label)
            .into_any_element()
    }

    fn mention_card(&mut self, message: &Message, due: Option<chrono::DateTime<chrono::Utc>>, cx: &mut Context<Self>) -> AnyElement {
        let (channel, id) = (message.channel_id, message.id);
        let place = {
            let store = &self.session.read(cx).store;
            store.channel(channel).map(|c| match c.guild_id.and_then(|g| store.guild(g)) {
                Some(guild) => format!("#{} \u{b7} {}", c.name.clone().unwrap_or_default(), guild.name),
                None => store.dm_name(c),
            })
        };
        let author = message.author.display_name().to_owned();
        let url = cdn::user_avatar(message.author.id, message.author.avatar.as_deref(), 64);
        let when = message.timestamp.with_timezone(&Local).format("%d/%m/%Y %H:%M").to_string();
        let full = crate::plain::plain_text(message, &self.session.read(cx).store);
        let text = if full.is_empty() {
            "(attachment or embed)".to_owned()
        } else {
            match full.char_indices().nth(PREVIEW_CHARS) {
                Some((end, _)) => format!("{}...", &full[..end]),
                None => full,
            }
        };

        div()
            .id(("mention", id.0))
            .mx_2()
            .p_2()
            .flex()
            .flex_col()
            .gap_1()
            .rounded(px(8.))
            .bg(rgb(theme::BG_CHAT))
            .border_1()
            .border_color(rgb(theme::BORDER))
            .cursor_pointer()
            .hover(|s| s.bg(rgb(theme::BG_HOVER)))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.go(channel, Some(id), cx)))
            .children(place.map(|p| {
                div().text_size(px(11.)).font_weight(FontWeight::SEMIBOLD).text_color(rgb(theme::TEXT_MUTED)).child(p)
            }))
            .children(due.map(|due| {
                div()
                    .text_size(px(11.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(theme::TEXT_MUTED))
                    .child(due_label(due))
            }))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(avatar(&self.media, &url, 24., &author, cx))
                    .child(div().truncate().font_weight(FontWeight::SEMIBOLD).text_color(rgb(theme::TEXT_BRIGHT)).child(author))
                    .child(div().flex_none().text_size(px(11.)).text_color(rgb(theme::TEXT_FAINT)).child(when)),
            )
            .child(div().text_size(px(13.)).text_color(rgb(theme::TEXT)).child(text))
            .into_any_element()
    }

    fn pending_row(&mut self, item: Pending, cx: &mut Context<Self>) -> AnyElement {
        let channel = item.channel;
        div()
            .id(("pending", channel.0))
            .mx_2()
            .h(px(44.))
            .px_2()
            .flex()
            .items_center()
            .gap_2()
            .rounded(px(theme::RADIUS))
            .cursor_pointer()
            .hover(|s| s.bg(rgb(theme::BG_HOVER)))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.go(channel, None, cx)))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(div().truncate().font_weight(FontWeight::MEDIUM).text_color(rgb(theme::TEXT_BRIGHT)).child(item.name))
                    .child(div().truncate().text_size(px(12.)).text_color(rgb(theme::TEXT_MUTED)).child(item.place)),
            )
            .children((item.mentions > 0).then(|| {
                div()
                    .px(px(6.))
                    .rounded_full()
                    .bg(rgb(theme::BADGE))
                    .text_color(rgb(theme::TEXT_BRIGHT))
                    .text_size(px(11.))
                    .font_weight(FontWeight::BOLD)
                    .child(item.mentions.to_string())
            }))
            .into_any_element()
    }
}

impl Render for Inbox {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut list = div().id("inbox-list").flex_1().min_h_0().overflow_y_scroll().flex().flex_col().gap_2().py_2();

        match self.tab {
            Tab::Mentions => match self.session.read(cx).mentions.clone() {
                Load::Ready(messages) if messages.is_empty() => {
                    list = list.child(empty_note("Nobody has mentioned you lately."));
                }
                Load::Ready(messages) => {
                    for message in &messages {
                        list = list.child(self.mention_card(message, None, cx));
                    }
                }
                Load::Failed(error) => list = list.child(empty_note(&format!("Could not load mentions. {error}"))),
                _ => list = list.child(empty_note("Loading...")),
            },
            Tab::ForYou => {
                let loaded = self.session.read(cx).account.notification_center.clone();
                match loaded {
                    Load::Ready(items) if items.is_empty() => {
                        list = list.child(empty_note("Nothing for you right now."));
                    }
                    Load::Ready(items) => {
                        for item in &items {
                            let title = item.get("title").and_then(|v| v.as_str()).unwrap_or("Notification").to_owned();
                            let body = item.get("body").and_then(|v| v.as_str()).unwrap_or("").to_owned();
                            list = list.child(empty_note(&format!("{title}: {body}")));
                        }
                    }
                    Load::Failed(error) => list = list.child(empty_note(&format!("Could not load. {error}"))),
                    _ => list = list.child(empty_note("Loading...")),
                }
            }
            Tab::Unreads => {
                let items = self.pending(cx);
                if items.is_empty() {
                    list = list.child(empty_note("You are all caught up."));
                }
                for item in items {
                    list = list.child(self.pending_row(item, cx));
                }
            }
            Tab::Bookmarks => match self.session.read(cx).saved_messages.clone() {
                Load::Ready(saved) => {
                    let messages: Vec<_> = saved
                        .into_iter()
                        .filter(|saved| saved.save_data.due_at.is_none())
                        .map(|saved| saved.message)
                        .collect();
                    if messages.is_empty() {
                        list = list.child(empty_note("No bookmarked messages."));
                    }
                    for message in &messages {
                        list = list.child(self.mention_card(message, None, cx));
                    }
                }
                Load::Failed(error) => {
                    list = list.child(empty_note(&format!("Could not load bookmarks. {error}")));
                }
                _ => list = list.child(empty_note("Loading...")),
            },
            Tab::Reminders => match self.session.read(cx).saved_messages.clone() {
                Load::Ready(saved) => {
                    let saved: Vec<_> = saved
                        .into_iter()
                        .filter(|saved| saved.save_data.due_at.is_some())
                        .collect();
                    if saved.is_empty() {
                        list = list.child(empty_note("No reminders."));
                    }
                    for saved in &saved {
                        list = list.child(self.mention_card(&saved.message, saved.save_data.due_at, cx));
                    }
                }
                Load::Failed(error) => {
                    list = list.child(empty_note(&format!("Could not load reminders. {error}")));
                }
                _ => list = list.child(empty_note("Loading...")),
            },
            Tab::Scheduled => match self.session.read(cx).scheduled_messages.clone() {
                Load::Ready(messages) if messages.is_empty() => {
                    list = list.child(empty_note("No scheduled messages."));
                }
                Load::Ready(messages) => {
                    for message in &messages {
                        list = list.child(self.mention_card(message, None, cx));
                    }
                }
                Load::Failed(error) => {
                    list = list.child(empty_note(&format!("Could not load scheduled messages. {error}")));
                }
                _ => list = list.child(empty_note("Loading...")),
            },
        }

        let header = div()
            .flex_none()
            .h(px(48.))
            .px_3()
            .flex()
            .items_center()
            .gap_2()
            .border_b_1()
            .border_color(rgb(theme::BORDER))
            .child(icon("inbox", 18., theme::TEXT_BRIGHT))
            .child(div().font_weight(FontWeight::BOLD).text_color(rgb(theme::TEXT_BRIGHT)).child("Inbox"))
            .child(div().flex_1())
            .child(self.tab_button(Tab::Mentions, "Mentions", cx))
            .child(self.tab_button(Tab::ForYou, "For You", cx))
            .child(self.tab_button(Tab::Unreads, "Unreads", cx))
            .child(self.tab_button(Tab::Bookmarks, "Bookmarks", cx))
            .child(self.tab_button(Tab::Reminders, "Reminders", cx))
            .child(self.tab_button(Tab::Scheduled, "Scheduled", cx))
            .children((self.tab == Tab::Unreads).then(|| {
                div()
                    .id("mark-all-read")
                    .size(px(28.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(4.))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(theme::BG_HOVER)))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.mark_all_read(cx)))
                    .child(icon("check-check", 16., theme::TEXT_MUTED))
            }));

        div()
            .id("inbox-overlay")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .child(
                div()
                    .id("inbox-scrim")
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .on_click(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(CloseInbox))),
            )
            .child(
                div()
                    .id("inbox-card")
                    .occlude()
                    .absolute()
                    .top(px(44.))
                    .right(px(8.))
                    .w(px(440.))
                    .h(px(560.))
                    .flex()
                    .flex_col()
                    .rounded(px(8.))
                    .border_1()
                    .border_color(rgb(theme::BORDER))
                    .bg(rgb(theme::BG_SIDEBAR))
                    .overflow_hidden()
                    .child(header)
                    .child(list),
            )
    }
}

fn due_label(due: chrono::DateTime<chrono::Utc>) -> String {
    let now = chrono::Utc::now();
    let seconds = (due - now).num_seconds();
    let (amount, unit, suffix) = if seconds >= 0 {
        let seconds = seconds as u64;
        let (amount, unit) = if seconds < 60 {
            (1, "minute")
        } else if seconds < 3600 {
            (seconds / 60, "minute")
        } else if seconds < 86400 {
            (seconds / 3600, "hour")
        } else {
            (seconds / 86400, "day")
        };
        (amount, unit, "from now")
    } else {
        let seconds = (-seconds) as u64;
        let (amount, unit) = if seconds < 60 {
            (1, "minute")
        } else if seconds < 3600 {
            (seconds / 60, "minute")
        } else if seconds < 86400 {
            (seconds / 3600, "hour")
        } else {
            (seconds / 86400, "day")
        };
        (amount, unit, "ago")
    };
    let plural = if amount == 1 { "" } else { "s" };
    format!("Due {} {}{} {}", amount, unit, plural, suffix)
}

fn empty_note(text: &str) -> AnyElement {
    div().p_4().text_size(px(13.)).text_color(rgb(theme::TEXT_MUTED)).child(text.to_owned()).into_any_element()
}
