//! The "Join a Server" dialog: paste an invite, see what it is, join.

use gpui_kit::{
    AppContext as _, ClickEvent, Context, Entity, EventEmitter, FontWeight, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _,
    Subscription, Window, div, px, rgb,
    component::input::{Input, InputEvent, InputState},
};
use oxicord_media::cdn;
use oxicord_model::invite_code;

use crate::{
    account::Load,
    backend::{Backend, Command},
    media::{Media, avatar},
    profile::modal,
    session::Session,
    theme,
};

pub struct CloseJoin;

impl EventEmitter<CloseJoin> for JoinServerView {}

pub struct JoinServerView {
    session: Entity<Session>,
    media: Entity<Media>,
    backend: Backend,
    code: Entity<InputState>,
    /// Waiting for the answer to a join we sent.
    joining: bool,
    _subscriptions: Vec<Subscription>,
}

impl JoinServerView {
    pub fn new(
        session: Entity<Session>,
        media: Entity<Media>,
        backend: Backend,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let code = cx.new(|cx| InputState::new(window, cx).placeholder("https://discord.gg/hTKzmak"));
        let subscriptions = vec![
            cx.observe(&session, |this, session, cx| {
                // A finished join closes the dialog, and the server shows up
                // through the gateway.
                if this.joining {
                    match session.read(cx).invite_joined {
                        Some(Ok(())) => {
                            this.joining = false;
                            cx.emit(CloseJoin);
                        }
                        Some(Err(_)) => this.joining = false,
                        None => {}
                    }
                }
                cx.notify();
            }),
            cx.observe(&media, |_, _, cx| cx.notify()),
            cx.subscribe_in(&code, window, |this, _, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::PressEnter { .. }) {
                    this.look_up(cx);
                }
            }),
        ];
        Self { session, media, backend, code, joining: false, _subscriptions: subscriptions }
    }

    /// Starts fresh each time the dialog opens.
    pub fn opened(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.joining = false;
        self.session.update(cx, |s, cx| {
            s.invite = Load::Idle;
            s.invite_joined = None;
            cx.notify();
        });
        self.code.update(cx, |input, cx| {
            input.set_value("", window, cx);
            input.focus(window, cx);
        });
    }

    fn look_up(&mut self, cx: &mut Context<Self>) {
        let text = self.code.read(cx).value().to_string();
        let Some(code) = invite_code(&text) else {
            self.session.update(cx, |s, cx| {
                s.invite = Load::Failed("That does not look like an invite.".into());
                cx.notify();
            });
            return;
        };
        self.session.update(cx, |s, cx| {
            s.invite = Load::Loading;
            s.invite_joined = None;
            cx.notify();
        });
        self.backend.send(Command::ResolveInvite(code));
    }

    fn join(&mut self, cx: &mut Context<Self>) {
        let session = self.session.read(cx);
        let (Load::Ready(preview), Some(session_id)) = (&session.invite, session.store.session_id()) else {
            return;
        };
        let command = Command::JoinInvite { code: preview.code.clone(), session_id: session_id.to_owned() };
        self.joining = true;
        self.session.update(cx, |s, _| s.invite_joined = None);
        self.backend.send(command);
        cx.notify();
    }
}

impl Render for JoinServerView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (invite, joined) = {
            let s = self.session.read(cx);
            (s.invite.clone(), s.invite_joined.clone())
        };

        let mut card = div()
            .id("join-card")
            .occlude()
            .w(px(440.))
            .p_5()
            .flex()
            .flex_col()
            .gap_3()
            .rounded(px(10.))
            .border_1()
            .border_color(rgb(theme::BORDER))
            .bg(rgb(theme::BG_SIDEBAR))
            .child(
                div()
                    .text_size(px(20.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(theme::TEXT_BRIGHT))
                    .child("Join a Server"),
            )
            .child(
                div()
                    .text_color(rgb(theme::TEXT_MUTED))
                    .child("Paste an invite link or code, then press Enter."),
            )
            .child(Input::new(&self.code));

        match invite {
            Load::Loading => {
                card = card.child(div().text_color(rgb(theme::TEXT_MUTED)).child("Looking that up..."));
            }
            Load::Failed(error) => {
                card = card.child(div().text_color(rgb(theme::DANGER)).child(error));
            }
            Load::Ready(preview) => {
                let icon_url = preview
                    .icon
                    .as_deref()
                    .zip(preview.guild_id)
                    .map(|(hash, id)| cdn::guild_icon(id, hash, 96))
                    .unwrap_or_default();
                let counts = match (preview.online, preview.members) {
                    (Some(on), Some(all)) => format!("{on} online \u{b7} {all} members"),
                    (None, Some(all)) => format!("{all} members"),
                    _ => String::new(),
                };
                card = card.child(
                    div()
                        .p_3()
                        .flex()
                        .items_center()
                        .gap_3()
                        .rounded(px(8.))
                        .bg(rgb(theme::BG_PANEL))
                        .child(avatar(&self.media, &icon_url, 48., &preview.guild_name, cx))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(rgb(theme::TEXT_BRIGHT))
                                        .child(preview.guild_name.clone()),
                                )
                                .child(
                                    div()
                                        .text_size(px(12.))
                                        .text_color(rgb(theme::TEXT_MUTED))
                                        .child(counts),
                                ),
                        ),
                );
                card = card.child(
                    div()
                        .id("join-button")
                        .h(px(36.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(6.))
                        .cursor_pointer()
                        .bg(rgb(theme::ACCENT))
                        .text_color(rgb(theme::TEXT_BRIGHT))
                        .hover(|s| s.opacity(0.85))
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.join(cx)))
                        .child(if self.joining { "Joining..." } else { "Join Server" }),
                );
            }
            _ => {}
        }
        if let Some(Err(error)) = joined {
            card = card.child(div().text_color(rgb(theme::DANGER)).child(error));
        }

        modal(
            card.into_any_element(),
            Box::new(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(CloseJoin))),
        )
    }
}
