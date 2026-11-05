use gpui_kit::{
    AppContext as _, Context, Entity, InteractiveElement as _, IntoElement, ParentElement as _,
    Render, SharedString, Styled as _, Subscription, Window, div, px, rgb,
    component::{
        Disableable as _,
        button::{Button, ButtonVariants as _},
        input::{Input, InputEvent, InputState},
    },
};

use crate::{
    backend::{Backend, Command},
    theme,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Busy {
    No,
    Browser,
    Token,
}

pub struct LoginView {
    backend: Backend,
    input: Entity<InputState>,
    busy: Busy,
    error: Option<SharedString>,
    _enter: Subscription,
}

impl LoginView {
    pub fn new(
        backend: Backend,
        notice: Option<SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .masked(true)
                .placeholder("Paste your token here")
        });
        let enter = cx.subscribe_in(&input, window, |this, _, event: &InputEvent, _, cx| {
            if matches!(event, InputEvent::PressEnter { .. }) {
                this.submit_token(cx);
            }
        });

        Self {
            backend,
            input,
            busy: Busy::No,
            error: notice,
            _enter: enter,
        }
    }

    /// Signs in with a token we already have, used for the saved one at
    /// startup.
    pub fn sign_in_with(&mut self, token: String, cx: &mut Context<Self>) {
        self.start(Busy::Token, Command::Login(token), cx);
    }

    pub fn fail(&mut self, message: String, cx: &mut Context<Self>) {
        self.busy = Busy::No;
        self.error = Some(message.into());
        cx.notify();
    }

    fn submit_token(&mut self, cx: &mut Context<Self>) {
        if self.busy != Busy::No {
            return;
        }
        let token = self.input.read(cx).value().trim().to_owned();
        if token.is_empty() {
            self.error = Some("Paste your token first.".into());
            cx.notify();
            return;
        }
        self.start(Busy::Token, Command::Login(token), cx);
    }

    fn open_browser(&mut self, cx: &mut Context<Self>) {
        if self.busy == Busy::No {
            self.start(Busy::Browser, Command::BrowserLogin, cx);
        }
    }

    fn start(&mut self, busy: Busy, command: Command, cx: &mut Context<Self>) {
        self.busy = busy;
        self.error = None;
        self.backend.send(command);
        cx.notify();
    }
}

impl Render for LoginView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let error = self.error.clone();
        let idle = self.busy == Busy::No;

        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgb(theme::BG_RAIL))
            .text_color(rgb(theme::TEXT))
            .child(
                div()
                    .id("login-card")
                    .w(px(400.))
                    .p_8()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .rounded(px(theme::RADIUS * 1.5))
                    .bg(rgb(theme::BG_CHAT))
                    .border_1()
                    .border_color(rgb(theme::BORDER))
                    .child(div().text_2xl().child("Welcome back"))
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(theme::TEXT_MUTED))
                            .child("Sign in on Discord's own page. Your password never reaches this app."),
                    )
                    .child(
                        Button::new("sign-in-browser")
                            .primary()
                            .label("Sign in with Discord")
                            .loading(self.busy == Busy::Browser)
                            .disabled(!idle)
                            .on_click(cx.listener(|this, _, _, cx| this.open_browser(cx))),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(theme::TEXT_MUTED))
                            .child("Or paste a token you already have"),
                    )
                    .child(Input::new(&self.input))
                    .children(error.map(|message| {
                        div()
                            .text_sm()
                            .text_color(rgb(theme::DANGER))
                            .child(message)
                    }))
                    .child(
                        Button::new("sign-in-token")
                            .label("Use token")
                            .loading(self.busy == Busy::Token)
                            .disabled(!idle)
                            .on_click(cx.listener(|this, _, _, cx| this.submit_token(cx))),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(theme::TEXT_MUTED))
                            .child("Your token is kept in the system credential manager."),
                    ),
            )
    }
}
