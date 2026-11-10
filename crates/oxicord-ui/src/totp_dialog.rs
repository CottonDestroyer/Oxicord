//! "Enable Authenticator App": the three steps the live client shows in one
//! modal (download an app, scan the code, enter the first code), then the
//! backup codes the server hands back. Measured from the live modal at
//! 1920 x 1080: a 480 wide frame with the same header as the password dialog,
//! rows 20 apart divided by 1 px lines, a 156 wide picture column and a 254
//! wide text column 20 beside it, and a 128 square code on a 144 white tile.
//! The backup codes screen could not be reached live without turning the
//! authenticator on, so it is built from the web client's wording.

use gpui_kit::{
    AnyElement, AppContext as _, ClickEvent, ClipboardItem, Context, Div, Entity, EventEmitter, FontWeight,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _,
    Subscription, Window, canvas, div, fill, img, point, px, rgb, rgba, size,
    component::input::{Input, InputState},
};
use gpui_kit::prelude::FluentBuilder as _;
use oxicord_model::BackupCode;

use crate::{
    AccountRequest,
    account::Load,
    backend::Backend,
    chat::markdown::rich,
    icons::icon,
    media::Media,
    profile::modal,
    session::Session,
    theme,
    totp::{self, Qr},
};

const LINK_TEXT: &str = "Download and install [Authy](https://www.authy.com/) or \
    [Google Authenticator](https://support.google.com/accounts/answer/1066447?hl=en) for your phone or tablet.";
const MONO: &str = "Consolas";

pub struct CloseTotp;

impl EventEmitter<CloseTotp> for TotpDialog {}

enum Phase {
    Setup,
    Codes(Vec<BackupCode>),
}

pub struct TotpDialog {
    session: Entity<Session>,
    media: Entity<Media>,
    backend: Backend,
    code: Entity<InputState>,
    secret: String,
    qr: Option<Qr>,
    phase: Phase,
    error: Option<String>,
    copied: bool,
    _subscription: Subscription,
}

impl TotpDialog {
    pub fn new(
        session: Entity<Session>,
        media: Entity<Media>,
        backend: Backend,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let code = cx.new(|cx| InputState::new(window, cx).placeholder("000 000"));
        // The server's answer moves to the backup codes or puts its complaint
        // on the first screen.
        let subscription = cx.observe(&session, |this, session, cx| match session.read(cx).account.totp.clone() {
            Load::Ready(codes) => {
                session.update(cx, |session, _| session.account.totp = Load::Idle);
                this.phase = Phase::Codes(codes);
                this.error = None;
                cx.notify();
            }
            Load::Failed(message) => {
                session.update(cx, |session, _| session.account.totp = Load::Idle);
                this.error = Some(message);
                cx.notify();
            }
            _ => cx.notify(),
        });
        Self {
            session,
            media,
            backend,
            code,
            secret: String::new(),
            qr: None,
            phase: Phase::Setup,
            error: None,
            copied: false,
            _subscription: subscription,
        }
    }

    /// Starts over with a fresh key, which is only real once it is activated.
    pub fn opened(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.secret = totp::new_secret();
        let account = {
            let session = self.session.read(cx);
            session
                .account
                .details
                .ready()
                .and_then(|details| details.email.clone())
                .or_else(|| session.store.me().map(|me| me.username.clone()))
                .unwrap_or_default()
        };
        self.qr = Qr::new(&totp::otpauth_uri(&account, &self.secret));
        self.phase = Phase::Setup;
        self.error = None;
        self.copied = false;
        self.code.update(cx, |input, cx| {
            input.set_value("", window, cx);
            input.focus(window, cx);
        });
        cx.notify();
    }

    /// The backup codes screen with made-up codes, for the dev command.
    pub fn preview_codes(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.opened(window, cx);
        let sample = ["a1b2c3d4", "e5f6a7b8", "c9d0e1f2", "a3b4c5d6", "e7f8a9b0", "c1d2e3f4", "a5b6c7d8", "e9f0a1b2"];
        let codes = sample
            .iter()
            .enumerate()
            .map(|(index, code)| BackupCode { code: (*code).to_owned(), consumed: index == 2 })
            .collect();
        self.phase = Phase::Codes(codes);
        cx.notify();
    }

    /// Leaving the backup codes also refreshes the account, whose
    /// two-factor line has just changed.
    fn close(&mut self, cx: &mut Context<Self>) {
        if matches!(self.phase, Phase::Codes(_)) {
            let backend = self.backend.clone();
            self.session
                .update(cx, |session, cx| session.request_account(&backend, AccountRequest::LoadDetails, cx));
        }
        cx.emit(CloseTotp);
    }

    /// The six digits, or what is wrong with them.
    fn digits(typed: &str) -> Result<String, &'static str> {
        let digits: String = typed.chars().filter(|c| !c.is_whitespace()).collect();
        if digits.len() == 6 && digits.chars().all(|c| c.is_ascii_digit()) {
            Ok(digits)
        } else {
            Err("Enter the 6-digit code from your authenticator app.")
        }
    }

    fn activate(&mut self, cx: &mut Context<Self>) {
        if matches!(self.session.read(cx).account.totp, Load::Loading) {
            return;
        }
        let typed = self.code.read(cx).value().to_string();
        match Self::digits(&typed) {
            Err(complaint) => {
                self.error = Some(complaint.to_owned());
                cx.notify();
            }
            Ok(code) => {
                self.error = None;
                let backend = self.backend.clone();
                let secret = totp::plain(&self.secret);
                self.session.update(cx, |session, cx| {
                    session.request_account(&backend, AccountRequest::EnableTotp { secret, code }, cx);
                });
                cx.notify();
            }
        }
    }

    fn heading(text: &'static str) -> Div {
        div()
            .text_size(px(14.))
            .line_height(px(18.))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(rgb(theme::TEXT_BRIGHT))
            .child(text)
    }

    fn body(text: &'static str) -> Div {
        div().text_size(px(16.)).line_height(px(20.)).text_color(rgb(theme::TEXT)).child(text)
    }

    /// One step: its picture in the left column, its words in the right.
    fn step(picture: AnyElement, words: Div, divided: bool, bottom: f32) -> Div {
        div()
            .mx(px(24.))
            .pt(px(20.))
            .pb(px(bottom))
            .flex()
            .items_center()
            .gap(px(20.))
            .when(divided, |row| row.border_t_1().border_color(rgba(0x97979f29)))
            .child(div().w(px(156.)).flex_none().flex().justify_center().child(picture))
            .child(words.w(px(254.)).flex_none().flex().flex_col())
    }

    fn qr_picture(&self) -> AnyElement {
        let Some(qr) = &self.qr else {
            return div().size(px(144.)).into_any_element();
        };
        let (count, dark) = (qr.size, qr.dark.clone());
        let code = canvas(
            |_, _, _| (),
            move |bounds, _, window, _| {
                let cell = f32::from(bounds.size.width) / count as f32;
                for (index, is_dark) in dark.iter().enumerate() {
                    if !*is_dark {
                        continue;
                    }
                    let (x, y) = ((index % count) as f32, (index / count) as f32);
                    // Whole pixels, so neighbouring modules leave no seams.
                    let (left, top) = ((x * cell).round(), (y * cell).round());
                    let (right, bottom) = (((x + 1.) * cell).round(), ((y + 1.) * cell).round());
                    let origin = point(bounds.origin.x + px(left), bounds.origin.y + px(top));
                    window.paint_quad(fill(
                        gpui_kit::Bounds { origin, size: size(px(right - left), px(bottom - top)) },
                        rgb(0x000000),
                    ));
                }
            },
        )
        .size(px(128.));
        div().size(px(144.)).flex_none().flex().items_center().justify_center().bg(rgb(0xffffff)).child(code).into_any_element()
    }

    fn header(title: &'static str, line: &'static str) -> Div {
        div()
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
                    .child(title),
            )
            .child(div().text_size(px(16.)).line_height(px(20.)).text_color(rgb(theme::TEXT_MUTED)).child(line))
    }

    fn button(id: &'static str, label: &'static str, primary: bool, width: f32) -> gpui_kit::Stateful<Div> {
        div()
            .id(id)
            .h(px(40.))
            .w(px(width))
            .flex()
            .flex_none()
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

    fn close_button(cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("close-totp")
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
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close(cx)))
            .child(icon("x", 20., theme::TEXT_MUTED))
    }

    fn frame() -> gpui_kit::Stateful<Div> {
        div()
            .id("totp-frame")
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
    }

    fn setup(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let saving = matches!(self.session.read(cx).account.totp, Load::Loading);
        let names = Default::default();
        let links = div()
            .text_size(px(16.))
            .line_height(px(20.))
            .text_color(rgb(theme::TEXT))
            .child(rich(LINK_TEXT, 0x2FA0, &self.media, &names, window, cx));

        let first = Self::step(
            img(gpui_kit::SharedString::from("mfa/authenticator.png")).size(px(100.)).into_any_element(),
            div().gap(px(0.)).child(Self::heading("Download an authenticator app")).child(links),
            false,
            20.,
        );
        let second = Self::step(
            self.qr_picture(),
            div()
                .child(Self::heading("Scan the QR code"))
                .child(Self::body("Open the authentication app and scan the image to the left using your phone's camera."))
                .child(div().mt(px(20.)).child(Self::heading("2FA Key (Manual entry)")))
                .child(
                    div()
                        .font_family(MONO)
                        .text_size(px(16.))
                        .line_height(px(20.))
                        .text_color(rgb(theme::TEXT))
                        .child(totp::spaced(&self.secret)),
                ),
            true,
            20.,
        );
        let activate = Self::button("totp-activate", "Activate", true, 100.)
            .opacity(if saving { 0.5 } else { 1. })
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.activate(cx)));
        let third = Self::step(
            img(gpui_kit::SharedString::from("mfa/phone-code.svg")).w(px(100.)).h(px(85.)).into_any_element(),
            div()
                .child(Self::heading("Log in with your code"))
                .child(Self::body("Enter the 6-digit verification code generated."))
                .child(
                    div()
                        .mt(px(9.))
                        .flex()
                        .gap(px(8.))
                        .child(div().w(px(146.)).h(px(40.)).child(Input::new(&self.code)))
                        .child(activate),
                )
                .children(self.error.clone().map(|message| {
                    div().mt(px(8.)).text_size(px(14.)).line_height(px(18.)).text_color(rgb(theme::DANGER)).child(message)
                })),
            true,
            28.,
        );

        div()
            .flex()
            .flex_col()
            .child(Self::header("Enable Authenticator App", "Make your account safer in 3 easy steps:"))
            .child(first)
            .child(second)
            .child(third)
    }

    fn codes(&mut self, codes: &[BackupCode], cx: &mut Context<Self>) -> Div {
        let list: Vec<String> = codes.iter().map(|c| c.code.clone()).collect();
        let cells = codes.iter().map(|entry| {
            div()
                .w(px(209.))
                .h(px(40.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(8.))
                .bg(rgba(0x97979f1f))
                .font_family(MONO)
                .text_size(px(16.))
                .line_height(px(20.))
                .text_color(rgb(if entry.consumed { theme::TEXT_MUTED } else { theme::TEXT_BRIGHT }))
                .when(entry.consumed, |cell| cell.line_through())
                .child(entry.code.clone())
        });
        let copied = self.copied;
        div()
            .flex()
            .flex_col()
            .child(Self::header(
                "Backup Codes",
                "Keep these somewhere safe. Each code signs you in once if you lose your authenticator app.",
            ))
            .child(div().px(px(24.)).pt(px(16.)).flex().flex_wrap().gap(px(12.)).children(cells))
            .child(
                div()
                    .px(px(24.))
                    .pt(px(24.))
                    .flex()
                    .gap(px(8.))
                    .child(
                        Self::button("totp-copy", if copied { "Copied" } else { "Copy Codes" }, false, 211.).on_click(
                            cx.listener(move |this, _: &ClickEvent, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(list.join("\n")));
                                this.copied = true;
                                cx.notify();
                            }),
                        ),
                    )
                    .child(
                        Self::button("totp-done", "Done", true, 211.)
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close(cx))),
                    ),
            )
    }
}

impl Render for TotpDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content = match &self.phase {
            Phase::Setup => self.setup(window, cx),
            Phase::Codes(codes) => {
                let codes = codes.clone();
                self.codes(&codes, cx)
            }
        };
        let frame = Self::frame().child(content).child(Self::close_button(cx));
        modal(frame.into_any_element(), Box::new(cx.listener(|this, _: &ClickEvent, _, cx| this.close(cx))))
    }
}

#[cfg(test)]
mod tests {
    use super::TotpDialog;

    #[test]
    fn the_first_code_is_six_digits_with_or_without_a_space() {
        assert_eq!(TotpDialog::digits("123 456"), Ok("123456".to_owned()));
        assert_eq!(TotpDialog::digits("123456"), Ok("123456".to_owned()));
        assert!(TotpDialog::digits("12345").is_err());
        assert!(TotpDialog::digits("12345a").is_err());
        assert!(TotpDialog::digits("").is_err());
    }
}
