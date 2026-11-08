use gpui_kit::{
    ClickEvent, Context, Entity, EventEmitter, InteractiveElement as _, IntoElement, ParentElement as _,
    Render, StatefulInteractiveElement as _, Styled as _, Window, div, px, rgb,
    component::input::{Input, InputState},
};
use gpui_kit::AppContext as _;
use oxicord_model::Snowflake;

use crate::{backend::{Backend, Command}, profile::modal, session::Session, theme};

pub struct CloseCreateServer;

impl EventEmitter<CloseCreateServer> for CreateServerDialog {}

pub struct CreateServerDialog {
    backend: Backend,
    name: Entity<InputState>,
    creating: bool,
}

impl CreateServerDialog {
    pub fn new(_session: Entity<Session>, backend: Backend, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let name = cx.new(|cx| InputState::new(window, cx).placeholder("My server"));
        Self { backend, name, creating: false }
    }

    pub fn opened(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.creating = false;
        self.name.update(cx, |input, cx| {
            input.set_value("", window, cx);
            input.focus(window, cx);
        });
        cx.notify();
    }

    fn create(&mut self, cx: &mut Context<Self>) {
        let name = self.name.read(cx).value().trim().to_owned();
        if name.is_empty() {
            return;
        }
        self.creating = true;
        self.backend.send(Command::CreateGuild { name });
        cx.emit(CloseCreateServer);
    }
}

impl Render for CreateServerDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let creating = self.creating;
        let card = div()
            .id("create-server-card")
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
            .child(
                div()
                    .text_size(px(18.))
                    .font_weight(gpui_kit::FontWeight::BOLD)
                    .text_color(rgb(theme::TEXT_BRIGHT))
                    .child("Create a server"),
            )
            .child(Input::new(&self.name))
            .child(
                div()
                    .id("create-server-confirm")
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
                    .child(if creating { "Creating..." } else { "Create" }),
            );
        modal(card.into_any_element(), Box::new(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(CloseCreateServer))))
    }
}