use gpui_kit::{
    AppContext as _, Context, Entity, InteractiveElement as _, IntoElement, ParentElement as _,
    Render, SharedString, Styled as _, Subscription, Window, div, px, rgb,
    component::{
        button::{Button, ButtonVariants as _},
        input::{Input, InputEvent, InputState},
    },
};

use crate::{
    backend::{Backend, Command},
    theme,
};

pub struct LoginView {
    backend: Backend,
    input: Entity<InputState>,
    busy: bool,
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
                this.submit(cx);
            }
        });

        Self {
            backend,
            input,
            busy: false,
            error: notice,
            _enter: enter,
        }
    }

    /// Signs in with a token we already have, used for the saved one at
    /// startup.
    pub fn sign_in_with(&mut self, token: String, cx: &mut Context<Self>) {
        self.begin(token, cx);
    }

    pub fn fail(&mut self, message: String, cx: &mut Context<Self>) {
        self.busy = false;
        self.error = Some(message.into());
        cx.notify();
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let token = self.input.read(cx).value().trim().to_owned();
        if token.is_empty() {
            self.error = Some("Paste your token first.".into());
            cx.notify();
            return;
        }
        self.begin(token, cx);
    }

    fn begin(&mut self, token: String, cx: &mut Context<Self>) {
        self.busy = true;
        self.error = None;
        self.backend.send(Command::Login(token));
        cx.notify();
    }
}

impl Render for LoginView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let error = self.error.clone();

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
                            .child("Paste your account token to sign in."),
                    )
                    .child(Input::new(&self.input))
                    .children(error.map(|message| {
                        div()
                            .text_sm()
                            .text_color(rgb(theme::DANGER))
                            .child(message)
                    }))
                    .child(
                        Button::new("sign-in")
                            .primary()
                            .label("Sign in")
                            .loading(self.busy)
                            .on_click(cx.listener(|this, _, _, cx| this.submit(cx))),
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
