//! The account dialogs that ask for proof before something drastic: disable
//! the account, delete it, or switch two-factor off. They share the live
//! client's layout, measured at 1920 x 1080: a 480 wide frame with a title, an
//! amber warning box (10 padding, radius 8), one required field 20 under it
//! and Cancel beside a red 211 by 40 button 44 under the field.

use gpui_kit::{
    AppContext as _, ClickEvent, Context, Entity, EventEmitter, FontWeight, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _, Subscription, Window, div, px, rgb,
    rgba,
    component::input::{Input, InputState},
};

use crate::{
    AccountRequest,
    account::Load,
    backend::Backend,
    icons::icon,
    profile::modal,
    session::Session,
    theme,
};

/// Which drastic thing the dialog is asking about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Danger {
    DisableAccount,
    DeleteAccount,
}

impl Danger {
    fn title(self) -> &'static str {
        match self {
            Self::DisableAccount => "Disable account",
            Self::DeleteAccount => "Delete account",
        }
    }

    fn warning(self) -> &'static str {
        match self {
            Self::DisableAccount => {
                "Are you sure that you want to disable your account? This will immediately log you out and make \
                 your account inaccessible to anyone."
            }
            Self::DeleteAccount => {
                "Are you sure that you want to delete your account? This will immediately log you out of your \
                 account and you will not be able to log in again."
            }
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::DisableAccount | Self::DeleteAccount => "Password",
        }
    }

    fn button(self) -> &'static str {
        match self {
            Self::DisableAccount => "Disable Account",
            Self::DeleteAccount => "Delete Account",
        }
    }

    fn missing(self) -> &'static str {
        match self {
            Self::DisableAccount | Self::DeleteAccount => "Enter your password to go on.",
        }
    }

    fn request(self, secret: String) -> AccountRequest {
        match self {
            Self::DisableAccount => AccountRequest::DisableAccount { password: secret },
            Self::DeleteAccount => AccountRequest::DeleteAccount { password: secret },
        }
    }
}

/// Asked of the session when a button on the account page wants the dialog.
pub struct OpenDanger(pub Danger);

impl EventEmitter<OpenDanger> for Session {}

pub struct CloseDanger;

impl EventEmitter<CloseDanger> for DangerDialog {}

pub struct DangerDialog {
    session: Entity<Session>,
    backend: Backend,
    kind: Danger,
    secret: Entity<InputState>,
    error: Option<String>,
    _subscription: Subscription,
}

impl DangerDialog {
    pub fn new(session: Entity<Session>, backend: Backend, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let secret = cx.new(|cx| InputState::new(window, cx).masked(true));
        let subscription = cx.observe(&session, |this, session, cx| match session.read(cx).account.danger.clone() {
            Load::Ready(()) => {
                session.update(cx, |session, _| session.account.danger = Load::Idle);
                cx.emit(CloseDanger);
            }
            Load::Failed(message) => {
                session.update(cx, |session, _| session.account.danger = Load::Idle);
                this.error = Some(message);
                cx.notify();
            }
            _ => cx.notify(),
        });
        Self { session, backend, kind: Danger::DisableAccount, secret, error: None, _subscription: subscription }
    }

    pub fn opened(&mut self, kind: Danger, window: &mut Window, cx: &mut Context<Self>) {
        self.kind = kind;
        self.error = None;
        self.secret.update(cx, |input, cx| {
            input.set_value("", window, cx);
            input.focus(window, cx);
        });
        cx.notify();
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        if matches!(self.session.read(cx).account.danger, Load::Loading) {
            return;
        }
        let typed = self.secret.read(cx).value().trim().to_string();
        if typed.is_empty() {
            self.error = Some(self.kind.missing().to_owned());
            cx.notify();
            return;
        }
        self.error = None;
        let (backend, request) = (self.backend.clone(), self.kind.request(typed));
        self.session.update(cx, |session, cx| session.request_account(&backend, request, cx));
        cx.notify();
    }

    fn footer_button(id: &'static str, label: &'static str, danger: bool) -> gpui_kit::Stateful<gpui_kit::Div> {
        div()
            .id(id)
            .h(px(40.))
            .w(px(211.))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(8.))
            .border_1()
            .border_color(if danger { rgba(0xffffff14) } else { rgba(0x97979f29) })
            .bg(if danger { rgb(0xd22d39) } else { rgba(0x97979f1f) })
            .cursor_pointer()
            .hover(|s| s.opacity(0.85))
            .text_size(px(16.))
            .line_height(px(20.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(if danger { rgb(0xffffff) } else { rgb(0xdcdcdf) })
            .child(label)
    }
}

impl Render for DangerDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let saving = matches!(self.session.read(cx).account.danger, Load::Loading);
        let kind = self.kind;

        let close = div()
            .id("close-danger")
            .absolute()
            .top(px(8.))
            .right(px(8.))
            .size(px(40.))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(8.))
            .cursor_pointer()
            .hover(|s| s.bg(rgb(theme::BG_HOVER)))
            .on_click(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(CloseDanger)))
            .child(icon("x", 20., theme::TEXT_MUTED));

        let body = div()
            .px(px(24.))
            .pb(px(8.))
            .flex()
            .flex_col()
            .child(
                div()
                    .mt(px(24.))
                    .p(px(10.))
                    .rounded(px(8.))
                    .bg(rgba(0xfaa61a14))
                    .text_size(px(16.))
                    .line_height(px(20.))
                    .text_color(rgb(0xce8000))
                    .child(kind.warning()),
            )
            .child(
                div()
                    .mt(px(20.))
                    .flex()
                    .gap(px(4.))
                    .text_size(px(16.))
                    .line_height(px(20.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(theme::TEXT_BRIGHT))
                    .child(kind.label())
                    .child(div().text_color(rgb(theme::DANGER)).child("*")),
            )
            .child(div().mt(px(8.)).h(px(40.)).child(Input::new(&self.secret).mask_toggle()))
            .child(
                div()
                    .h(px(20.))
                    .text_size(px(14.))
                    .line_height(px(20.))
                    .text_color(rgb(theme::DANGER))
                    .children(self.error.clone()),
            )
            .child(
                div()
                    .mt(px(24.))
                    .flex()
                    .gap(px(8.))
                    .child(
                        Self::footer_button("danger-cancel", "Cancel", false)
                            .on_click(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(CloseDanger))),
                    )
                    .child(
                        Self::footer_button("danger-confirm", kind.button(), true)
                            .opacity(if saving { 0.5 } else { 1. })
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.submit(cx))),
                    ),
            );

        let frame = div()
            .id("danger-frame")
            .occlude()
            .relative()
            .w(px(480.))
            .pt(px(8.))
            .pb(px(16.))
            .flex()
            .flex_col()
            .rounded(px(12.))
            .border_1()
            .border_color(rgb(theme::BORDER_OVERLAY))
            .bg(rgb(theme::BG_PANEL))
            .child(
                div()
                    .px(px(24.))
                    .pt(px(16.))
                    .text_size(px(20.))
                    .line_height(px(24.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(theme::TEXT_BRIGHT))
                    .child(kind.title()),
            )
            .child(body)
            .child(close);

        modal(frame.into_any_element(), Box::new(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(CloseDanger))))
    }
}

#[cfg(test)]
mod tests {
    use super::Danger;
    use crate::AccountRequest;

    #[test]
    fn disabling_the_account_sends_the_password() {
        let AccountRequest::DisableAccount { password } = Danger::DisableAccount.request("hunter2".into()) else {
            panic!("wrong request");
        };
        assert_eq!(password, "hunter2");
    }

    #[test]
    fn deleting_the_account_sends_the_password_too() {
        let AccountRequest::DeleteAccount { password } = Danger::DeleteAccount.request("hunter2".into()) else {
            panic!("wrong request");
        };
        assert_eq!(password, "hunter2");
        assert_eq!(Danger::DeleteAccount.title(), "Delete account");
        assert!(Danger::DeleteAccount.warning().contains("delete your account"));
    }
}
