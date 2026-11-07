//! The panel on the right that lists pinned messages or search results.

use chrono::Local;
use gpui_kit::{
    AnyElement, AppContext as _, Context, Entity, EventEmitter, FontWeight, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _,
    Subscription, Window, div, px, rgb,
    component::input::{Input, InputEvent, InputState},
};
use oxicord_media::cdn;
use oxicord_model::{Channel, Message, Permissions, Snowflake};

use crate::{
    account::Load,
    backend::Backend,
    icons::icon,
    media::{Media, avatar},
    session::{PanelKind, Session},
    theme,
};

/// Longest stretch of a message shown in a result card.
const PREVIEW_CHARS: usize = 280;

pub struct ClosePanel;

impl EventEmitter<ClosePanel> for SidePanel {}

pub struct SidePanel {
    session: Entity<Session>,
    media: Entity<Media>,
    backend: Backend,
    kind: PanelKind,
    camera: bool,
    screen_share: bool,
    activity: bool,
    guild: Option<Snowflake>,
    channel: Option<Snowflake>,
    query: Entity<InputState>,
    _subscriptions: Vec<Subscription>,
}

impl SidePanel {
    pub fn new(
        session: Entity<Session>,
        media: Entity<Media>,
        backend: Backend,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let query = cx.new(|cx| InputState::new(window, cx).placeholder("Search messages"));
        let subscriptions = vec![
            cx.observe(&session, |_, _, cx| cx.notify()),
            cx.observe(&media, |_, _, cx| cx.notify()),
            cx.subscribe_in(&query, window, |this, _, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::PressEnter { .. }) {
                    this.submit(cx);
                }
            }),
        ];
        Self {
            session,
            media,
            backend,
            kind: PanelKind::Pins,
            camera: false,
            screen_share: false,
            activity: false,
            guild: None,
            channel: None,
            query,
            _subscriptions: subscriptions,
        }
    }

    pub fn kind(&self) -> PanelKind {
        self.kind
    }

    pub fn show(
        &mut self,
        kind: PanelKind,
        guild: Option<Snowflake>,
        channel: Option<Snowflake>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.kind = kind;
        self.guild = guild;
        self.channel = channel;
        match kind {
            PanelKind::Pins => self.load_pins(cx),
            PanelKind::Threads => self.load_threads(cx),
            PanelKind::Search => self.query.update(cx, |input, cx| input.focus(window, cx)),
            PanelKind::VoiceDetails => {}
        }
        cx.notify();
    }

    /// The open channel changed. Pins follow it, a search keeps its results
    /// but its next run uses the new place.
    pub fn follow(&mut self, guild: Option<Snowflake>, channel: Option<Snowflake>, cx: &mut Context<Self>) {
        self.guild = guild;
        if self.channel != channel {
            self.channel = channel;
            if self.kind == PanelKind::Pins {
                self.load_pins(cx);
            } else if self.kind == PanelKind::Threads {
                self.load_threads(cx);
            }
        }
    }

    /// The channel whose threads we list: the open one, or the parent when a
    /// thread is open.
    fn thread_parent(&self, cx: &gpui_kit::App) -> Option<Snowflake> {
        let channel = self.channel?;
        let store = &self.session.read(cx).store;
        match store.channel(channel) {
            Some(c) if c.kind.is_thread() => c.parent_id,
            _ => Some(channel),
        }
    }

    fn load_threads(&mut self, cx: &mut Context<Self>) {
        let Some(parent) = self.thread_parent(cx) else { return };
        let backend = self.backend.clone();
        self.session
            .update(cx, |session, cx| session.request_threads(&backend, parent, cx));
    }

    fn load_pins(&mut self, cx: &mut Context<Self>) {
        let Some(channel) = self.channel else { return };
        let backend = self.backend.clone();
        self.session
            .update(cx, |session, cx| session.request_pins(&backend, channel, cx));
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        let query = self.query.read(cx).value().trim().to_owned();
        let Some(channel) = self.channel else { return };
        if query.is_empty() || self.kind != PanelKind::Search {
            return;
        }
        let (backend, guild) = (self.backend.clone(), self.guild);
        self.session
            .update(cx, |session, cx| session.start_search(&backend, guild, channel, query, cx));
    }

    fn can_unpin(&self, channel: Snowflake, cx: &gpui_kit::App) -> bool {
        let store = &self.session.read(cx).store;
        match store.channel(channel) {
            Some(c) if c.kind.is_dm() => true,
            Some(c) => store.permissions_in(c).contains(Permissions::MANAGE_MESSAGES),
            None => false,
        }
    }

    fn card(&mut self, message: &Message, show_channel: bool, cx: &mut Context<Self>) -> AnyElement {
        let (channel, id) = (message.channel_id, message.id);
        let author = message.author.display_name().to_owned();
        let url = cdn::user_avatar(message.author.id, message.author.avatar.as_deref(), 64);
        let when = message.timestamp.with_timezone(&Local).format("%d/%m/%Y %H:%M").to_string();

        let place = show_channel
            .then(|| {
                let store = &self.session.read(cx).store;
                store.channel(channel).map(|c| {
                    if c.kind.is_dm() {
                        store.dm_name(c)
                    } else {
                        format!("#{}", c.name.clone().unwrap_or_default())
                    }
                })
            })
            .flatten();

        let text = if message.content.is_empty() {
            match message.attachments.len() {
                0 => "(no text)".to_owned(),
                1 => "1 attachment".to_owned(),
                n => format!("{n} attachments"),
            }
        } else {
            let full = crate::plain::plain_text(message, &self.session.read(cx).store);
            match full.char_indices().nth(PREVIEW_CHARS) {
                Some((end, _)) => format!("{}...", &full[..end]),
                None => full,
            }
        };

        let unpin = (self.kind == PanelKind::Pins && self.can_unpin(channel, cx)).then(|| {
            div()
                .id(("unpin", id.0))
                .size(px(26.))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(4.))
                .cursor_pointer()
                .hover(|s| s.bg(rgb(theme::BG_ACTIVE)))
                .on_click(cx.listener(move |this, _, _, cx| {
                    let backend = this.backend.clone();
                    this.session
                        .update(cx, |s, cx| s.set_pinned(&backend, channel, id, false, cx));
                }))
                .child(icon("pin-off", 14., theme::TEXT_MUTED))
        });

        div()
            .id(("result", id.0))
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
            .on_click(cx.listener(move |this, _, _, cx| {
                this.session.update(cx, |s, cx| s.jump_to_message(channel, id, cx));
            }))
            .children(place.map(|p| {
                div()
                    .text_size(px(11.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(theme::TEXT_MUTED))
                    .child(p)
            }))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(avatar(&self.media, &url, 24., &author, cx))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .items_baseline()
                            .gap_2()
                            .child(
                                div()
                                    .truncate()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(theme::TEXT_BRIGHT))
                                    .child(author),
                            )
                            .child(
                                div()
                                    .flex_none()
                                    .text_size(px(11.))
                                    .text_color(rgb(theme::TEXT_FAINT))
                                    .child(when),
                            ),
                    )
                    .children(unpin),
            )
            .child(
                div()
                    .text_size(px(13.))
                    .text_color(rgb(theme::TEXT))
                    .child(text),
            )
            .into_any_element()
    }

    fn heading(text: &str) -> AnyElement {
        div()
            .px_4()
            .pt_1()
            .text_size(px(11.))
            .font_weight(FontWeight::BOLD)
            .text_color(rgb(theme::TEXT_MUTED))
            .child(text.to_uppercase())
            .into_any_element()
    }

    /// One thread in the list: its name, how busy it is, and when it last
    /// saw a message.
    fn thread_card(&mut self, thread: &Channel, cx: &mut Context<Self>) -> AnyElement {
        let id = thread.id;
        let busy = thread.message_count.map_or(String::new(), |n| {
            format!("{n} message{}", if n == 1 { "" } else { "s" })
        });
        let last = thread
            .last_message_id
            .map(|m| m.datetime().with_timezone(&Local).format("%d/%m/%Y %H:%M").to_string())
            .unwrap_or_default();

        div()
            .id(("thread", id.0))
            .mx_2()
            .p_2()
            .flex()
            .flex_col()
            .gap(px(2.))
            .rounded(px(8.))
            .bg(rgb(theme::BG_CHAT))
            .border_1()
            .border_color(rgb(theme::BORDER))
            .cursor_pointer()
            .hover(|s| s.bg(rgb(theme::BG_HOVER)))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.session.update(cx, |s, cx| s.jump_to(id, cx));
            }))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(icon("messages-square", 14., theme::TEXT_MUTED))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(theme::TEXT_BRIGHT))
                            .child(thread.name.clone().unwrap_or_default()),
                    ),
            )
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(rgb(theme::TEXT_MUTED))
                    .child(format!("{busy}  {last}").trim().to_owned()),
            )
            .into_any_element()
    }

    fn note(text: impl Into<String>) -> AnyElement {
        div()
            .p_4()
            .text_size(px(13.))
            .text_color(rgb(theme::TEXT_MUTED))
            .child(text.into())
            .into_any_element()
    }

    fn body(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        match self.kind {
            PanelKind::Pins => {
                let state = self
                    .channel
                    .and_then(|c| self.session.read(cx).pins.get(&c).cloned());
                match state {
                    None | Some(Load::Idle | Load::Loading) => vec![Self::note("Loading pins...")],
                    Some(Load::Failed(error)) => vec![Self::note(format!("Couldn't load pins. {error}"))],
                    Some(Load::Ready(pins)) if pins.is_empty() => {
                        vec![Self::note("This channel has no pinned messages yet.")]
                    }
                    Some(Load::Ready(pins)) => pins.iter().map(|m| self.card(m, false, cx)).collect(),
                }
            }
            PanelKind::Threads => {
                let Some(parent) = self.thread_parent(cx) else {
                    return vec![Self::note("Open a channel first.")];
                };
                let (open, archived) = {
                    let session = self.session.read(cx);
                    let open: Vec<Channel> =
                        session.store.open_threads(parent).into_iter().cloned().collect();
                    (open, session.archived_threads.get(&parent).cloned())
                };
                let mut rows = Vec::new();
                if open.is_empty() && !matches!(archived, Some(Load::Ready(_))) {
                    rows.push(Self::note("Loading threads..."));
                }
                if !open.is_empty() {
                    rows.push(Self::heading("Active"));
                    rows.extend(open.iter().map(|t| self.thread_card(t, cx)));
                }
                match archived {
                    Some(Load::Ready(page)) => {
                        let old: Vec<&Channel> =
                            page.threads.iter().filter(|t| t.archived()).collect();
                        if !old.is_empty() {
                            rows.push(Self::heading("Archived"));
                            rows.extend(old.into_iter().map(|t| self.thread_card(t, cx)));
                        }
                        if rows.is_empty() {
                            rows.push(Self::note("No threads here yet."));
                        }
                    }
                    Some(Load::Failed(error)) => {
                        rows.push(Self::note(format!("Couldn't load archived threads. {error}")));
                    }
                    _ => {}
                }
                rows
            }
            PanelKind::Search => {
                let state = self.session.read(cx).search.clone();
                match state {
                    None => vec![Self::note(
                        "Type something and press Enter. You can narrow it with from:name, in:channel, has:image or mentions:name.",
                    )],
                    Some((_, Load::Idle | Load::Loading)) => vec![Self::note("Searching...")],
                    Some((_, Load::Failed(error))) => {
                        vec![Self::note(format!("Search failed. {error}"))]
                    }
                    Some((_, Load::Ready(page))) if page.messages.is_empty() => {
                        vec![Self::note("No results.")]
                    }
                    Some((_, Load::Ready(page))) => {
                        let mut rows = vec![div()
                            .px_4()
                            .pb_1()
                            .text_size(px(12.))
                            .text_color(rgb(theme::TEXT_MUTED))
                            .child(format!("{} results", page.total))
                            .into_any_element()];
                        rows.extend(page.messages.iter().map(|m| self.card(m, true, cx)));
                        rows
                    }
                }
            }
            PanelKind::VoiceDetails => {
                let call = self.session.read(cx).voice.clone();
                let Some((_, call_channel)) = call.target else {
                    return vec![Self::note("You are not in a call. Click a voice channel to join it.")];
                };
                let channel_name = {
                    let store = &self.session.read(cx).store;
                    store.channel(call_channel).map_or_else(
                        || "Voice Channel".to_owned(),
                        |c| c.name.clone().unwrap_or_else(|| store.dm_name(c)),
                    )
                };
                let participants: Vec<AnyElement> = {
                    let store = &self.session.read(cx).store;
                    store
                        .voice_in(call_channel)
                        .iter()
                        .map(|voice| {
                            let name = store
                                .user(voice.user_id)
                                .map(|u| u.display_name().to_owned())
                                .unwrap_or_else(|| voice.user_id.to_string());
                            let suffix = if voice.self_deaf || voice.deaf {
                                " (deafened)"
                            } else if voice.self_mute || voice.mute {
                                " (muted)"
                            } else if call.speaking.contains(&voice.user_id) {
                                " (speaking)"
                            } else {
                                ""
                            };
                            Self::note(format!("{}{}", name, suffix))
                        })
                        .collect()
                };
                let mut rows = vec![Self::note(if call.connected {
                    format!("Connected to {channel_name}")
                } else {
                    format!("Connecting to {channel_name}...")
                })];
                if call.encrypted {
                    rows.push(Self::note(match &call.code {
                        Some(code) => format!("End-to-end encrypted. Verification code: {code}"),
                        None => "End-to-end encrypted.".to_owned(),
                    }));
                }
                rows.extend(call.warnings.iter().map(|text| Self::note(text.clone())));
                if participants.is_empty() {
                    rows.push(Self::note("No one else is here."));
                } else {
                    rows.extend(participants);
                }
                rows.extend([
                    {
                        let camera = self.camera;
                        div()
                            .id("toggle-camera")
                            .px_4()
                            .py_2()
                            .rounded(px(4.))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(theme::BG_HOVER)))
                            .on_click(cx.listener(|this, _: &gpui_kit::ClickEvent, _, cx| {
                                this.camera = !this.camera;
                                cx.notify();
                            }))
                            .child(if camera { "Turn Off Camera" } else { "Turn On Camera" })
                            .into_any_element()
                    },
                    {
                        let sharing = self.screen_share;
                        div()
                            .id("toggle-screen-share")
                            .px_4()
                            .py_2()
                            .rounded(px(4.))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(theme::BG_HOVER)))
                            .on_click(cx.listener(|this, _: &gpui_kit::ClickEvent, _, cx| {
                                this.screen_share = !this.screen_share;
                                cx.notify();
                            }))
                            .child(if sharing { "Stop Sharing" } else { "Share Your Screen" })
                            .into_any_element()
                    },
                    {
                        let active = self.activity;
                        div()
                            .id("toggle-activity")
                            .px_4()
                            .py_2()
                            .rounded(px(4.))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(theme::BG_HOVER)))
                            .on_click(cx.listener(|this, _: &gpui_kit::ClickEvent, _, cx| {
                                this.activity = !this.activity;
                                cx.notify();
                            }))
                            .child(if active { "Stop Activity" } else { "Start An Activity" })
                            .into_any_element()
                    },
                    div()
                        .id("toggle-mute")
                        .px_4()
                        .py_2()
                        .rounded(px(4.))
                        .cursor_pointer()
                        .hover(|s| s.bg(rgb(theme::BG_HOVER)))
                        .on_click(cx.listener(|this, _: &gpui_kit::ClickEvent, _, cx| {
                            let backend = this.backend.clone();
                            this.session.update(cx, |s, cx| s.toggle_voice_mute(&backend, cx));
                        }))
                        .child(if call.muted { "Unmute" } else { "Mute" })
                        .into_any_element(),
                    div()
                        .id("toggle-deafen")
                        .px_4()
                        .py_2()
                        .rounded(px(4.))
                        .cursor_pointer()
                        .hover(|s| s.bg(rgb(theme::BG_HOVER)))
                        .on_click(cx.listener(|this, _: &gpui_kit::ClickEvent, _, cx| {
                            let backend = this.backend.clone();
                            this.session.update(cx, |s, cx| s.toggle_voice_deafen(&backend, cx));
                        }))
                        .child(if call.deafened { "Undeafen" } else { "Deafen" })
                        .into_any_element(),
                    div()
                        .id("disconnect-voice")
                        .px_4()
                        .py_2()
                        .rounded(px(4.))
                        .cursor_pointer()
                        .hover(|s| s.bg(rgb(theme::BG_HOVER)))
                        .on_click(cx.listener(|this, _: &gpui_kit::ClickEvent, _, cx| {
                            let backend = this.backend.clone();
                            this.session.update(cx, |s, cx| s.leave_voice(&backend, cx));
                            cx.emit(ClosePanel);
                        }))
                        .child("Disconnect")
                        .into_any_element(),
                ]);
                rows
            }
        }
    }
}

impl Render for SidePanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let title = match self.kind {
            PanelKind::Pins => "Pinned Messages",
            PanelKind::Search => "Search",
            PanelKind::Threads => "Threads",
            PanelKind::VoiceDetails => "Voice Details",
        };
        let rows = self.body(cx);

        div()
            .w(px(theme::MEMBERS_WIDTH + 100.))
            .h_full()
            .flex_none()
            .flex()
            .flex_col()
            .bg(rgb(theme::BG_MEMBERS))
            .border_l_1()
            .border_color(rgb(theme::BORDER))
            .child(
                div()
                    .h(px(theme::CHANNEL_HEADER_HEIGHT))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_4()
                    .border_b_1()
                    .border_color(rgb(theme::BORDER))
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(theme::TEXT_BRIGHT))
                            .child(title),
                    )
                    .child(
                        div()
                            .id("close-panel")
                            .size(px(28.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(4.))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(theme::BG_HOVER)))
                            .on_click(cx.listener(|_, _, _, cx| cx.emit(ClosePanel)))
                            .child(icon("x", 16., theme::TEXT_MUTED)),
                    ),
            )
            .children((self.kind == PanelKind::Search).then(|| div().p_2().child(Input::new(&self.query))))
            .child(
                div()
                    .id("panel-body")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .py_2()
                    .children(rows),
            )
    }
}
