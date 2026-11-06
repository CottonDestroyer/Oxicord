//! The "Set your status" dialog: the words and emoji next to your name, and
//! how long they stay.

use chrono::{DateTime, Duration, Local};
use gpui_kit::{
    AppContext as _, ClickEvent, Context, Entity, EventEmitter, FontWeight, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _,
    Subscription, Window, div, px, rgb,
    component::input::{Input, InputState},
};
use gpui_kit::prelude::FluentBuilder as _;
use oxicord_model::Emoji;
use oxicord_settings::CustomStatus;

use crate::{
    backend::Backend,
    emoji_panel::{EmojiPanel, Picked},
    icons::icon,
    media::{Media, avatar},
    profile::modal,
    session::Session,
    theme,
};

/// What a custom status may hold, as the official client limits it.
const MAX_CHARS: usize = 128;

pub struct CloseStatusDialog;

impl EventEmitter<CloseStatusDialog> for StatusDialog {}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ClearAfter {
    Day,
    FourHours,
    Hour,
    HalfHour,
    Never,
}

impl ClearAfter {
    const ALL: [Self; 5] = [Self::Day, Self::FourHours, Self::Hour, Self::HalfHour, Self::Never];

    fn span(self) -> Option<Duration> {
        match self {
            Self::Day => Some(Duration::hours(24)),
            Self::FourHours => Some(Duration::hours(4)),
            Self::Hour => Some(Duration::hours(1)),
            Self::HalfHour => Some(Duration::minutes(30)),
            Self::Never => None,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Day => "24 hours",
            Self::FourHours => "4 hours",
            Self::Hour => "1 hour",
            Self::HalfHour => "30 minutes",
            Self::Never => "Don't clear",
        }
    }
}

/// "tomorrow at 2:48 PM", "6:48 PM", as the dropdown spells the moment.
fn when(at: DateTime<Local>, now: DateTime<Local>) -> String {
    let time = at.format("%-I:%M %p");
    if at.date_naive() == now.date_naive() { time.to_string() } else { format!("tomorrow at {time}") }
}

pub struct StatusDialog {
    session: Entity<Session>,
    media: Entity<Media>,
    backend: Backend,
    text: Entity<InputState>,
    emoji: Entity<EmojiPanel>,
    emoji_open: bool,
    picked: Option<Emoji>,
    clear: ClearAfter,
    clear_open: bool,
    _subscriptions: Vec<Subscription>,
}

impl StatusDialog {
    pub fn new(
        session: Entity<Session>,
        media: Entity<Media>,
        backend: Backend,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let text = cx.new(|cx| InputState::new(window, cx).placeholder("What\u{2019}s your hot take?"));
        let emoji = {
            let (session, media, backend) = (session.clone(), media.clone(), backend.clone());
            cx.new(|cx| EmojiPanel::new(session, media, backend, window, cx).emoji_only())
        };
        let subscriptions = vec![
            cx.observe(&text, |_, _, cx| cx.notify()),
            cx.observe(&media, |_, _, cx| cx.notify()),
            cx.observe(&session, |_, _, cx| cx.notify()),
            cx.subscribe(&emoji, |this, _, picked: &Picked, cx| {
                this.picked = Some(picked.0.clone());
                this.emoji_open = false;
                cx.notify();
            }),
        ];
        Self {
            session,
            media,
            backend,
            text,
            emoji,
            emoji_open: false,
            picked: None,
            clear: ClearAfter::Day,
            clear_open: false,
            _subscriptions: subscriptions,
        }
    }

    /// Starts from the status in effect, so editing is a small change.
    pub fn opened(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let now = chrono::Utc::now().timestamp_millis().max(0) as u64;
        let current = self.session.read(cx).account.custom_status(now);
        self.emoji_open = false;
        self.clear_open = false;
        self.clear = if current.as_ref().is_some_and(|s| s.expires_at_ms.is_none()) {
            ClearAfter::Never
        } else {
            ClearAfter::Day
        };
        self.picked = current.as_ref().and_then(|s| {
            (s.emoji_id.is_some() || s.emoji_name.is_some()).then(|| Emoji {
                id: s.emoji_id.map(oxicord_model::Snowflake),
                name: s.emoji_name.clone(),
                ..Emoji::default()
            })
        });
        let words = current.map(|s| s.text).unwrap_or_default();
        self.text.update(cx, |input, cx| {
            input.set_value(words, window, cx);
            input.focus(window, cx);
        });
        cx.notify();
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        let text: String = self.text.read(cx).value().trim().chars().take(MAX_CHARS).collect();
        let status = CustomStatus {
            text,
            emoji_id: self.picked.as_ref().and_then(|e| e.id).map(|id| id.0),
            emoji_name: self.picked.as_ref().and_then(|e| e.name.clone()),
            expires_at_ms: self.clear.span().map(|span| {
                u64::try_from((chrono::Utc::now() + span).timestamp_millis()).unwrap_or(0)
            }),
        };
        let backend = self.backend.clone();
        self.session.update(cx, |s, cx| s.set_custom_status(&backend, Some(&status), cx));
        cx.emit(CloseStatusDialog);
    }

    /// The text and emoji as they will show on the card.
    fn bubble(&self, cx: &Context<Self>) -> String {
        let words = self.text.read(cx).value().to_string();
        let glyph = self.picked.as_ref().filter(|e| e.id.is_none()).and_then(|e| e.name.clone());
        [glyph, Some(words).filter(|w| !w.is_empty())].into_iter().flatten().collect::<Vec<_>>().join(" ")
    }
}

impl Render for StatusDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (name, username, avatar_url, status) = {
            let session = self.session.read(cx);
            match session.store.me() {
                Some(me) => (
                    me.display_name().to_owned(),
                    me.username.clone(),
                    oxicord_media::cdn::user_avatar(me.id, me.avatar.as_deref(), 128),
                    crate::main_view::dot_status(session.status),
                ),
                None => Default::default(),
            }
        };
        let bubble = self.bubble(cx);
        let has_text = !bubble.is_empty();
        let now = Local::now();
        let clear_label = match self.clear.span() {
            Some(span) => format!("Clear {}", when(now + span, now)),
            None => "Don't clear".to_owned(),
        };
        let can_save = self.session.read(cx).account.settings.ready().is_some();

        // The same card the account popout shows, with the status bubble
        // as it will read.
        let preview = div()
            .w(px(300.))
            .rounded(px(theme::RADIUS))
            .border_1()
            .border_color(rgb(theme::BORDER_MUTED))
            .bg(rgb(theme::BG_MENU))
            .overflow_hidden()
            .child(div().h(px(105.)).bg(rgb(theme::ACCENT)))
            .child(
                div()
                    .relative()
                    .px(px(16.))
                    .pb(px(16.))
                    .child(
                        div()
                            .relative()
                            .mt(px(-44.))
                            .w(px(80.))
                            .child(avatar(&self.media, &avatar_url, 80., &name, cx))
                            .child(
                                div()
                                    .absolute()
                                    .right(px(0.))
                                    .bottom(px(0.))
                                    .p(px(3.))
                                    .rounded_full()
                                    .bg(rgb(theme::BG_MENU))
                                    .child(crate::member_list::status_shape(status, 12., theme::BG_MENU)),
                            ),
                    )
                    .child(
                        div()
                            .absolute()
                            .left(px(109.))
                            .top(px(-7.))
                            .h(px(36.))
                            .max_w(px(179.))
                            .px(px(12.))
                            .flex()
                            .items_center()
                            .rounded(px(16.))
                            .border_1()
                            .border_color(rgb(theme::BORDER_MUTED))
                            .bg(rgb(0x17181b))
                            .text_size(px(14.))
                            .text_color(rgb(if has_text { theme::TEXT } else { theme::TEXT_FAINT }))
                            .child(div().truncate().child(if has_text { bubble } else { "What\u{2019}s your hot take?".to_owned() })),
                    )
                    .child(
                        div()
                            .mt(px(4.))
                            .text_size(px(20.))
                            .line_height(px(24.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(theme::TEXT))
                            .child(name),
                    )
                    .child(
                        div()
                            .text_size(px(14.))
                            .line_height(px(24.))
                            .text_color(rgb(theme::TEXT_BRIGHT))
                            .child(username),
                    ),
            );

        let glyph = match &self.picked {
            Some(e) if e.id.is_none() => div().text_size(px(20.)).child(e.name.clone().unwrap_or_default()).into_any_element(),
            _ => icon("face-slightly-smiling", 24., theme::TEXT_MUTED).into_any_element(),
        };

        let field = div()
            .h(px(48.))
            .px(px(12.))
            .flex()
            .items_center()
            .gap(px(8.))
            .rounded(px(theme::RADIUS))
            .border_1()
            .border_color(rgb(theme::ACCENT))
            .bg(rgb(theme::BG_INPUT))
            .child(
                div()
                    .id("status-emoji")
                    .size(px(28.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                        this.emoji_open = !this.emoji_open;
                        this.clear_open = false;
                        if this.emoji_open {
                            this.emoji.update(cx, |panel, cx| panel.show(None, window, cx));
                        }
                        cx.notify();
                    }))
                    .child(glyph),
            )
            .child(div().flex_1().child(Input::new(&self.text).appearance(false).bordered(false)))
            .when(self.picked.is_some() || has_text, |row| {
                row.child(
                    div()
                        .id("status-clear")
                        .size(px(20.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                            this.picked = None;
                            this.text.update(cx, |input, cx| input.set_value("", window, cx));
                            cx.notify();
                        }))
                        .child(icon("x", 16., theme::TEXT_MUTED)),
                )
            });

        let mut menu = div()
            .absolute()
            .bottom(px(48.))
            .right(px(0.))
            .w(px(272.))
            .p(px(8.))
            .flex()
            .flex_col()
            .rounded(px(theme::RADIUS))
            .border_1()
            .border_color(rgb(theme::BORDER_MUTED))
            .bg(rgb(theme::BG_MENU))
            .child(
                div()
                    .h(px(36.))
                    .px(px(8.))
                    .flex()
                    .items_center()
                    .text_size(px(14.))
                    .text_color(rgb(theme::TEXT_FAINT))
                    .child("Clear after"),
            );
        for option in ClearAfter::ALL {
            let label = match option.span() {
                Some(span) => format!("{} ({})", option.name(), when(now + span, now)),
                None => option.name().to_owned(),
            };
            let chosen = option == self.clear;
            menu = menu.child(
                div()
                    .id(option.name())
                    .h(px(36.))
                    .px(px(8.))
                    .flex()
                    .items_center()
                    .justify_between()
                    .rounded(px(4.))
                    .cursor_pointer()
                    .text_size(px(14.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(if chosen { theme::TEXT_BRIGHT } else { theme::TEXT_MUTED }))
                    .bg(rgb(if chosen { theme::BG_ACTIVE } else { theme::BG_MENU }))
                    .hover(|s| s.bg(rgb(theme::BG_HOVER)).text_color(rgb(theme::TEXT_BRIGHT)))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.clear = option;
                        this.clear_open = false;
                        cx.notify();
                    }))
                    .child(label)
                    .when(chosen, |row| {
                        row.child(div().size(px(16.)).rounded_full().bg(rgb(theme::ACCENT)).flex().items_center().justify_center().child(icon("check", 12., 0xffffff)))
                    }),
            );
        }

        let footer = div()
            .relative()
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .id("clear-after")
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .cursor_pointer()
                    .text_size(px(16.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(theme::TEXT))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.clear_open = !this.clear_open;
                        this.emoji_open = false;
                        cx.notify();
                    }))
                    .child(clear_label)
                    .child(icon(if self.clear_open { "chevron-up" } else { "chevron-down" }, 16., theme::TEXT_MUTED)),
            )
            .child(
                div()
                    .id("status-save")
                    .w(px(100.))
                    .h(px(40.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(theme::RADIUS))
                    .text_size(px(14.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(0xffffff))
                    .bg(rgb(theme::ACCENT))
                    .when(can_save, |b| {
                        b.cursor_pointer()
                            .hover(|s| s.opacity(0.85))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.save(cx)))
                    })
                    .when(!can_save, |b| b.opacity(0.5))
                    .child("Save"),
            )
            .when(self.clear_open, |f| f.child(menu));

        let card = div()
            .id("status-card")
            .occlude()
            .relative()
            .w(px(480.))
            .flex()
            .flex_col()
            .gap(px(24.))
            .p(px(24.))
            .rounded(px(theme::RADIUS))
            .border_1()
            .border_color(rgb(theme::BORDER_MUTED))
            .bg(rgb(theme::BG_PANEL))
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
                            .child("Set your status"),
                    )
                    .child(
                        div()
                            .id("status-close")
                            .size(px(24.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .on_click(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(CloseStatusDialog)))
                            .child(icon("x", 24., theme::TEXT_MUTED)),
                    ),
            )
            .child(div().flex().justify_center().child(preview))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(
                        div()
                            .text_size(px(16.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(theme::TEXT_BRIGHT))
                            .child("Status"),
                    )
                    .child(field),
            )
            .child(footer)
            .when(self.emoji_open, |card| {
                card.child(div().absolute().left(px(488.)).top(px(0.)).child(self.emoji.clone()))
            });

        modal(
            card.into_any_element(),
            Box::new(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(CloseStatusDialog))),
        )
    }
}
