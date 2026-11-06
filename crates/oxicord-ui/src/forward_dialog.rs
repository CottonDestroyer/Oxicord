//! The "Forward To" dialog: pick up to five conversations or channels, add
//! an optional note, and send a copy of a message to each.

use gpui_kit::{
    AnyElement, AppContext as _, ClickEvent, Context, Entity, EventEmitter, FontWeight, InteractiveElement as _,
    IntoElement, KeyDownEvent, ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _,
    Subscription, Window, div, px, rgb,
    component::input::{Input, InputEvent, InputState},
};
use gpui_kit::prelude::FluentBuilder as _;
use oxicord_model::Snowflake;

use crate::{
    backend::{Backend, Command},
    icons::icon,
    media::{Media, avatar},
    profile::modal,
    session::Session,
    switcher::{self, Goal, Hit},
    theme,
};

/// The most places one message can be forwarded to at once.
const MAX_TARGETS: usize = 5;

pub struct CloseForward;

impl EventEmitter<CloseForward> for ForwardDialog {}

pub struct ForwardDialog {
    session: Entity<Session>,
    media: Entity<Media>,
    backend: Backend,
    search: Entity<InputState>,
    note: Entity<InputState>,
    hits: Vec<Hit>,
    /// The message being forwarded: its channel, server and id.
    source: Option<(Snowflake, Option<Snowflake>, Snowflake)>,
    chosen: Vec<Snowflake>,
    _subscriptions: Vec<Subscription>,
}

/// A hit's conversation or channel, if it is one we can post to now.
fn target(hit: &Hit) -> Option<Snowflake> {
    match hit.goal {
        Goal::Dm(id) | Goal::Channel(id) => Some(id),
        Goal::User(_) | Goal::Guild(_) => None,
    }
}

impl ForwardDialog {
    pub fn new(
        session: Entity<Session>,
        media: Entity<Media>,
        backend: Backend,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search for channels or conversations"));
        let note = cx.new(|cx| InputState::new(window, cx).placeholder("Add an optional message"));
        let subscriptions = vec![
            cx.subscribe(&search, |this, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    this.refresh(cx);
                }
            }),
            cx.subscribe(&note, |this, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::PressEnter { .. }) {
                    this.send(cx);
                }
            }),
            cx.observe(&media, |_, _, cx| cx.notify()),
        ];
        Self {
            session,
            media,
            backend,
            search,
            note,
            hits: Vec::new(),
            source: None,
            chosen: Vec::new(),
            _subscriptions: subscriptions,
        }
    }

    pub fn opened(
        &mut self,
        channel: Snowflake,
        guild: Option<Snowflake>,
        message: Snowflake,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.source = Some((channel, guild, message));
        self.chosen.clear();
        self.note.update(cx, |input, cx| input.set_value("", window, cx));
        self.search.update(cx, |input, cx| {
            input.set_value("", window, cx);
            input.focus(window, cx);
        });
        self.refresh(cx);
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        let query = self.search.read(cx).value().to_string();
        self.hits = switcher::search(&self.session.read(cx).store, &query)
            .into_iter()
            .filter(|hit| target(hit).is_some())
            .collect();
        cx.notify();
    }

    fn toggle(&mut self, id: Snowflake, cx: &mut Context<Self>) {
        if let Some(at) = self.chosen.iter().position(|c| *c == id) {
            self.chosen.remove(at);
        } else if self.chosen.len() < MAX_TARGETS {
            self.chosen.push(id);
        }
        cx.notify();
    }

    fn send(&mut self, cx: &mut Context<Self>) {
        let Some((from, guild, message)) = self.source else { return };
        if self.chosen.is_empty() {
            return;
        }
        let note = self.note.read(cx).value().trim().to_owned();
        self.backend.send(Command::ForwardMessage {
            from,
            guild,
            message,
            targets: std::mem::take(&mut self.chosen),
            note,
        });
        cx.emit(CloseForward);
    }
}

impl Render for ForwardDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut list = div().id("forward-results").max_h(px(300.)).overflow_y_scroll().flex().flex_col();
        for (index, hit) in self.hits.clone().into_iter().enumerate() {
            let Some(id) = target(&hit) else { continue };
            let picked = self.chosen.contains(&id);
            let leading: AnyElement = match &hit.picture {
                Some(url) => avatar(&self.media, url, 32., &hit.title, cx).into_any_element(),
                None => div()
                    .size(px(32.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(icon(hit.glyph, 20., theme::TEXT_MUTED))
                    .into_any_element(),
            };
            list = list.child(
                div()
                    .id(("forward-hit", index as u64))
                    .h(px(44.))
                    .px(px(8.))
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .rounded(px(theme::RADIUS))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(theme::BG_HOVER)))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.toggle(id, cx)))
                    .child(leading)
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgb(theme::TEXT_BRIGHT))
                            .child(hit.title.clone()),
                    )
                    .child(div().truncate().text_size(px(12.)).text_color(rgb(theme::TEXT_MUTED)).child(hit.note.clone()))
                    .child(
                        div()
                            .size(px(20.))
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(4.))
                            .border_2()
                            .border_color(rgb(if picked { theme::ACCENT } else { theme::TEXT_FAINT }))
                            .bg(rgb(if picked { theme::ACCENT } else { theme::BG_PANEL }))
                            .when(picked, |b| b.child(icon("check", 14., 0xffffff))),
                    ),
            );
        }

        let ready = !self.chosen.is_empty();
        let card = div()
            .id("forward-card")
            .occlude()
            .w(px(440.))
            .p(px(16.))
            .flex()
            .flex_col()
            .gap(px(12.))
            .rounded(px(theme::RADIUS))
            .border_1()
            .border_color(rgb(theme::BORDER_MUTED))
            .bg(rgb(theme::BG_PANEL))
            .capture_key_down(cx.listener(|_, event: &KeyDownEvent, _, cx| {
                if event.keystroke.key == "escape" {
                    cx.emit(CloseForward);
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
                            .child("Forward To"),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(rgb(theme::TEXT_MUTED))
                            .child(format!("{} / {MAX_TARGETS}", self.chosen.len())),
                    ),
            )
            .child(Input::new(&self.search))
            .child(list)
            .child(Input::new(&self.note))
            .child(
                div().flex().justify_end().child(
                    div()
                        .id("forward-send")
                        .h(px(40.))
                        .px(px(24.))
                        .flex()
                        .items_center()
                        .rounded(px(theme::RADIUS))
                        .text_size(px(14.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(rgb(0xffffff))
                        .bg(rgb(theme::ACCENT))
                        .when(ready, |b| {
                            b.cursor_pointer()
                                .hover(|s| s.opacity(0.85))
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.send(cx)))
                        })
                        .when(!ready, |b| b.opacity(0.5))
                        .child("Send"),
                ),
            );

        modal(card.into_any_element(), Box::new(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(CloseForward))))
    }
}
