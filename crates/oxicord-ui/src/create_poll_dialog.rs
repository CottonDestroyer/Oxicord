use gpui_kit::{
    ClickEvent, Context, Entity, EventEmitter, InteractiveElement as _, IntoElement, ParentElement as _,
    Render, StatefulInteractiveElement as _, Styled as _, Window, div, px, rgb,
    component::input::{Input, InputState},
};
use gpui_kit::AppContext as _;
use oxicord_model::Snowflake;

use crate::{backend::{Backend, Command}, profile::modal, session::Session, theme};

pub struct CloseCreatePoll;

impl EventEmitter<CloseCreatePoll> for CreatePollDialog {}

pub struct CreatePollDialog {
    backend: Backend,
    channel: Option<Snowflake>,
    guild: Option<Snowflake>,
    question: Entity<InputState>,
    answers: Vec<Entity<InputState>>,
    multiselect: bool,
}

impl CreatePollDialog {
    pub fn new(_session: Entity<Session>, backend: Backend, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let question = cx.new(|cx| InputState::new(window, cx).placeholder("Ask a question"));
        Self { backend, channel: None, guild: None, question, answers: vec![], multiselect: false }
    }

    pub fn opened(&mut self, channel: Snowflake, guild: Option<Snowflake>, window: &mut Window, cx: &mut Context<Self>) {
        self.channel = Some(channel);
        self.guild = guild;
        self.multiselect = false;
        self.question.update(cx, |i, cx| { i.set_value("", window, cx); i.focus(window, cx); });
        self.answers = vec![
            cx.new(|cx| InputState::new(window, cx).placeholder("Answer 1")),
            cx.new(|cx| InputState::new(window, cx).placeholder("Answer 2")),
        ];
        cx.notify();
    }

    fn create(&mut self, cx: &mut Context<Self>) {
        let Some(channel) = self.channel else { return };
        let question = self.question.read(cx).value().trim().to_owned();
        if question.is_empty() { return; }
        let answers: Vec<String> = self.answers.iter().map(|a| a.read(cx).value().trim().to_owned()).filter(|s| !s.is_empty()).collect();
        if answers.len() < 2 { return; }
        self.backend.send(Command::SendMessage {
            channel,
            guild: self.guild,
            content: String::new(),
            reply_to: None,
            ping_reply: true,
            files: Vec::new(),
            stickers: Vec::new(),
            poll: Some((question, answers, 24, self.multiselect)),
        });
        cx.emit(CloseCreatePoll);
    }
}

impl Render for CreatePollDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let card = div()
            .id("create-poll-card")
            .occlude()
            .w(px(440.))
            .p(px(24.))
            .flex()
            .flex_col()
            .gap(px(12.))
            .rounded(px(theme::RADIUS))
            .border_1()
            .border_color(rgb(theme::BORDER_MUTED))
            .bg(rgb(theme::BG_PANEL))
            .child(div().text_size(px(18.)).font_weight(gpui_kit::FontWeight::BOLD).text_color(rgb(theme::TEXT_BRIGHT)).child("Create Poll"))
            .child(Input::new(&self.question))
            .children(self.answers.iter().map(|a| Input::new(a).into_any_element()))
            .child(
                div()
                    .id("create-poll-confirm")
                    .h(px(36.))
                    .px(px(16.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(theme::RADIUS))
                    .bg(rgb(theme::ACCENT))
                    .text_color(rgb(theme::TEXT_BRIGHT))
                    .cursor_pointer()
                    .hover(|s| s.opacity(0.85))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.create(cx)))
                    .child("Create Poll"),
            );
        modal(card.into_any_element(), Box::new(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(CloseCreatePoll))))
    }
}