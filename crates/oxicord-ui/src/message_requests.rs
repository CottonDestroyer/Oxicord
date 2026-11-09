use gpui_kit::{
    ClickEvent, Context, Entity, InteractiveElement as _, IntoElement, ParentElement as _, Render,
    StatefulInteractiveElement as _, Styled as _, Subscription, Window, div, px, rgb,
};
use oxicord_media::cdn;
use oxicord_model::{Channel, ChannelType, Snowflake};
use oxicord_store::Store;

use crate::{
    backend::{Backend, Command},
    chat::format::message_time,
    icons::icon,
    media::{Media, avatar},
    session::{DmOpened, Session},
    theme,
};

/// The message requests page: conversations from people we have not
/// accepted yet. The official page module was never captured, so only the
/// data behind the rows is confirmed; the layout is our own.
pub struct MessageRequestsView {
    session: Entity<Session>,
    media: Entity<Media>,
    backend: Backend,
    tab: MrTab,
    _subscriptions: Vec<Subscription>,
}

/// The two lists the client keeps: requests and its spam inbox.
#[derive(Clone, Copy, PartialEq)]
enum MrTab {
    Requests,
    Spam,
}

impl MrTab {
    const ALL: [MrTab; 2] = [MrTab::Requests, MrTab::Spam];

    fn label(self) -> &'static str {
        match self {
            MrTab::Requests => "Message Requests",
            MrTab::Spam => "Spam",
        }
    }
}

struct RequestRow {
    id: Snowflake,
    name: String,
    avatar: String,
    /// When the request arrived, or its last message is newer.
    when: chrono::DateTime<chrono::Utc>,
}

/// The newer of the request time and the last message, the client's sort
/// key for a request row.
fn activity(c: &Channel) -> chrono::DateTime<chrono::Utc> {
    let message = c.last_message_id.map(|id| id.datetime()).unwrap_or_default();
    let requested = c
        .is_message_request_timestamp
        .as_deref()
        .and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok())
        .map(|t| t.with_timezone(&chrono::Utc))
        .unwrap_or_default();
    requested.max(message)
}

impl MessageRequestsView {
    pub fn new(
        session: Entity<Session>,
        media: Entity<Media>,
        backend: Backend,
        cx: &mut Context<Self>,
    ) -> Self {
        let _subscriptions = vec![cx.observe(&session, |_, _, cx| cx.notify())];
        Self { session, media, backend, tab: MrTab::Requests, _subscriptions }
    }

    fn rows(&self, cx: &gpui_kit::App) -> Vec<RequestRow> {
        let store = &self.session.read(cx).store;
        match self.tab {
            MrTab::Requests => {
                store.message_requests().into_iter().map(|c| Self::row(store, c)).collect()
            }
            MrTab::Spam => store.spam_channels().into_iter().map(|c| Self::row(store, c)).collect(),
        }
    }

    fn row(store: &Store, c: &Channel) -> RequestRow {
        let user = c.recipients.first();
        RequestRow {
            id: c.id,
            name: store.dm_name(c),
            avatar: match (c.kind, user) {
                (ChannelType::Dm, Some(user)) => {
                    cdn::user_avatar(user.id, user.avatar.as_deref(), 64)
                }
                _ => match c.icon.as_deref() {
                    Some(hash) => cdn::group_icon(c.id, hash, 64),
                    None => cdn::default_avatar(c.id),
                },
            },
            when: activity(c),
        }
    }

    fn render_row(&mut self, row: &RequestRow, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let id = row.id;
        let hour12 = self
            .session
            .read(cx)
            .account
            .pref(oxicord_settings::Key::HourCycle)
            .as_number()
            == 1;
        let offset = *chrono::Local::now().offset();
        let tag = message_time(row.when, chrono::Utc::now(), offset, hour12);
        div()
            .id(("message-request", id.0))
            .h(px(42.))
            .flex()
            .items_center()
            .gap(px(12.))
            .px(px(8.))
            .mx(px(8.))
            .rounded(px(theme::RADIUS))
            .cursor_pointer()
            .font_weight(gpui_kit::FontWeight::MEDIUM)
            .text_color(rgb(theme::TEXT_FAINT))
            .hover(|s| s.bg(rgb(theme::BG_HOVER)).text_color(rgb(theme::TEXT_BRIGHT)))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.session.update(cx, |_, cx| cx.emit(DmOpened(id)));
            }))
            .child(avatar(&self.media, &row.avatar, 32., &row.name, cx))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(div().truncate().text_color(rgb(theme::TEXT)).child(row.name.clone()))
                    .child(
                        div()
                            .flex_none()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(rgb(theme::TEXT_MUTED))
                                    .child(tag),
                            )
                            .child(self.accept_button(id, cx))
                            .child(self.ignore_button(id, cx)),
                    ),
            )
    }

    /// Accepts a request: the row hides at once, the way the client's
    /// accepted-optimistic set works, and the request goes out behind it.
    fn accept_button(&mut self, id: Snowflake, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        div()
            .id(("accept", id.0))
            .size(px(24.))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(theme::RADIUS))
            .cursor_pointer()
            .hover(|s| s.bg(rgb(theme::BG_ACTIVE)))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.session.update(cx, |s, _| s.store.accept_request_optimistic(id));
                this.backend.send(Command::AcceptMessageRequest { channel: id });
                cx.notify();
            }))
            .child(icon("check", 14., theme::TEXT_MUTED))
    }

    /// Ignores a request. The server closes the conversation and answers
    /// with a CHANNEL_DELETE.
    fn ignore_button(&mut self, id: Snowflake, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        div()
            .id(("ignore", id.0))
            .size(px(24.))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(theme::RADIUS))
            .cursor_pointer()
            .hover(|s| s.bg(rgb(theme::BG_ACTIVE)))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, _| {
                this.backend.send(Command::IgnoreMessageRequest { channel: id });
            }))
            .child(icon("x", 14., theme::TEXT_MUTED))
    }

    fn render_tabs(&mut self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let mut tabs = div().flex().items_center().gap_1();
        for tab in MrTab::ALL {
            let selected = self.tab == tab;
            tabs = tabs.child(
                div()
                    .id(tab.label())
                    .h(px(28.))
                    .px_3()
                    .flex()
                    .items_center()
                    .rounded(px(theme::RADIUS))
                    .cursor_pointer()
                    .text_size(px(14.))
                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                    .text_color(rgb(if selected { theme::TEXT_BRIGHT } else { theme::TEXT_MUTED }))
                    .bg(rgb(if selected { theme::BG_ACTIVE } else { theme::BG_CHAT }))
                    .hover(|s| s.bg(rgb(theme::BG_HOVER)))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.tab = tab;
                        cx.notify();
                    }))
                    .child(tab.label()),
            );
        }
        tabs
    }
}

impl Render for MessageRequestsView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let rows = self.rows(cx);
        let mut body = div().id("message-requests-body").flex_1().min_h_0().overflow_y_scroll();
        for row in rows {
            body = body.child(self.render_row(&row, cx));
        }
        div()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .flex_col()
            .bg(rgb(theme::BG_CHAT))
            .child(
                div()
                    .h(px(48.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap_4()
                    .px(px(16.))
                    .text_size(px(16.))
                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                    .text_color(rgb(theme::TEXT))
                    .child(self.render_tabs(cx)),
            )
            .child(div().flex_1().min_h_0().flex().flex_col().child(body))
    }
}
