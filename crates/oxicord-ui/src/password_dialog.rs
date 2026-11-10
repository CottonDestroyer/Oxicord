//! The "Update your password" dialog, opened from the account page. Measured
//! from the live client: a 480 wide frame (radius 12, 1 px border) with a 16 /
//! 24 / 8 header, three 430 wide fields 20 apart and a row of two 211 wide
//! buttons 24 under the last field.

use gpui_kit::{
    AppContext as _, ClickEvent, Context, Entity, EventEmitter, FontWeight, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _, Subscription, Window, div, px, rgb,
    rgba,
    component::input::{Input, InputState},
};

use crate::{
    account::Load,
    backend::Backend,
    icons::icon,
    profile::modal,
    session::Session,
    theme, AccountRequest,
};

pub struct ClosePassword;

impl EventEmitter<ClosePassword> for PasswordDialog {}

pub struct PasswordDialog {
    session: Entity<Session>,
    backend: Backend,
    current: Entity<InputState>,
    new: Entity<InputState>,
    confirm: Entity<InputState>,
    /// What stopped the last Done, ours or the server's.
    error: Option<String>,
    _subscription: Subscription,
}

impl PasswordDialog {
    pub fn new(session: Entity<Session>, backend: Backend, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let field = |cx: &mut Context<Self>, window: &mut Window| {
            cx.new(|cx| InputState::new(window, cx).masked(true))
        };
        let (current, new, confirm) = (field(cx, window), field(cx, window), field(cx, window));
        // The server's answer closes the dialog or puts its complaint on it.
        let subscription = cx.observe(&session, |this, session, cx| {
            match session.read(cx).account.password.clone() {
                Load::Ready(()) => {
                    session.update(cx, |session, _| session.account.password = Load::Idle);
                    cx.emit(ClosePassword);
                }
                Load::Failed(message) => {
                    this.error = Some(message);
                    session.update(cx, |session, _| session.account.password = Load::Idle);
                    cx.notify();
                }
                _ => cx.notify(),
            }
        });
        Self { session, backend, current, new, confirm, error: None, _subscription: subscription }
    }

    pub fn opened(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.error = None;
        for input in [&self.new, &self.confirm] {
            input.update(cx, |input, cx| input.set_value("", window, cx));
        }
        self.current.update(cx, |input, cx| {
            input.set_value("", window, cx);
            input.focus(window, cx);
        });
        cx.notify();
    }

    /// What is wrong with the three fields, if anything.
    fn complaint(current: &str, new: &str, confirm: &str) -> Option<&'static str> {
        if current.is_empty() || new.is_empty() || confirm.is_empty() {
            Some("All three passwords are required.")
        } else if new != confirm {
            Some("The new passwords do not match.")
        } else {
            None
        }
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        if matches!(self.session.read(cx).account.password, Load::Loading) {
            return;
        }
        let (current, new, confirm) = (
            self.current.read(cx).value().to_string(),
            self.new.read(cx).value().to_string(),
            self.confirm.read(cx).value().to_string(),
        );
        if let Some(complaint) = Self::complaint(&current, &new, &confirm) {
            self.error = Some(complaint.to_owned());
            cx.notify();
            return;
        }
        self.error = None;
        let backend = self.backend.clone();
        self.session.update(cx, |session, cx| {
            session.request_account(&backend, AccountRequest::ChangePassword { current, new }, cx);
        });
        cx.notify();
    }

    fn label(text: &'static str) -> impl IntoElement {
        div()
            .flex()
            .gap(px(4.))
            .text_size(px(16.))
            .line_height(px(20.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(rgb(theme::TEXT_BRIGHT))
            .child(text)
            .child(div().text_color(rgb(theme::DANGER)).child("*"))
    }

    fn field(&self, label: &'static str, input: &Entity<InputState>, top: f32) -> impl IntoElement {
        div()
            .mt(px(top))
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(Self::label(label))
            .child(div().h(px(40.)).child(Input::new(input).mask_toggle()))
    }

    fn footer_button(id: &'static str, label: &'static str, primary: bool) -> gpui_kit::Stateful<gpui_kit::Div> {
        div()
            .id(id)
            .h(px(40.))
            .w(px(211.))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(8.))
            .border_1()
            .border_color(if primary { rgba(0xffffff14) } else { rgba(0x97979f29) })
            .bg(if primary { rgb(theme::ACCENT) } else { rgba(0x97979f1f) })
            .cursor_pointer()
            .hover(|s| s.opacity(0.85))
            .text_size(px(16.))
            .line_height(px(20.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(if primary { rgb(0xffffff) } else { rgb(0xdcdcdf) })
            .child(label)
    }
}

impl Render for PasswordDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let saving = matches!(self.session.read(cx).account.password, Load::Loading);

        let header = div()
            .px(px(24.))
            .pt(px(16.))
            .pb(px(8.))
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(
                div()
                    .text_size(px(20.))
                    .line_height(px(24.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(theme::TEXT_BRIGHT))
                    .child("Update your password"),
            )
            .child(
                div()
                    .text_size(px(16.))
                    .line_height(px(20.))
                    .text_color(rgb(theme::TEXT_MUTED))
                    .child("Enter your current password and a new password."),
            );

        let close = div()
            .id("close-password")
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
            .on_click(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(ClosePassword)))
            .child(icon("x", 20., theme::TEXT_MUTED));

        let body = div()
            .px(px(24.))
            .pb(px(8.))
            .flex()
            .flex_col()
            .child(self.field("Current Password", &self.current, 16.))
            .child(self.field("New Password", &self.new, 20.))
            .child(self.field("Confirm New Password", &self.confirm, 20.))
            .children(self.error.clone().map(|message| {
                div()
                    .mt(px(12.))
                    .text_size(px(14.))
                    .line_height(px(18.))
                    .text_color(rgb(theme::DANGER))
                    .child(message)
            }))
            .child(
                div()
                    .mt(px(24.))
                    .flex()
                    .gap(px(8.))
                    .child(
                        Self::footer_button("password-cancel", "Cancel", false)
                            .on_click(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(ClosePassword))),
                    )
                    .child(
                        Self::footer_button("password-done", "Done", true)
                            .opacity(if saving { 0.5 } else { 1. })
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.submit(cx))),
                    ),
            );

        let frame = div()
            .id("password-frame")
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
            .child(header)
            .child(body)
            .child(close);

        modal(frame.into_any_element(), Box::new(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(ClosePassword))))
    }
}

#[cfg(test)]
mod tests {
    use super::PasswordDialog;

    #[test]
    fn the_fields_are_checked_before_anything_is_sent() {
        assert!(PasswordDialog::complaint("", "a", "a").is_some());
        assert!(PasswordDialog::complaint("old", "a", "").is_some());
        assert_eq!(
            PasswordDialog::complaint("old", "a", "b"),
            Some("The new passwords do not match.")
        );
        assert_eq!(PasswordDialog::complaint("old", "same", "same"), None);
    }
}
