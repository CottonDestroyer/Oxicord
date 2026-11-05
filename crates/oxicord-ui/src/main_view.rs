use gpui_kit::{
    Context, Entity, IntoElement, ParentElement as _, Render, Styled as _, Subscription, Window,
    div, px, rgb,
    component::button::Button,
};
use oxicord_gateway::ConnectionState;

use crate::{
    backend::{Backend, Command},
    session::Session,
    theme,
};

/// The signed in frame. The guild rail, channel list, chat and member list
/// replace the placeholders as they get built.
pub struct MainView {
    session: Entity<Session>,
    backend: Backend,
    _observe: Subscription,
}

impl MainView {
    pub fn new(session: Entity<Session>, backend: Backend, cx: &mut Context<Self>) -> Self {
        let observe = cx.observe(&session, |_, _, cx| cx.notify());
        Self {
            session,
            backend,
            _observe: observe,
        }
    }
}

impl Render for MainView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let session = self.session.read(cx);
        let name = session
            .store
            .me()
            .map(|u| u.display_name().to_owned())
            .unwrap_or_default();
        let status = connection_label(&session.connection);
        let backend = self.backend.clone();

        div()
            .size_full()
            .flex()
            .bg(rgb(theme::BG_RAIL))
            .text_color(rgb(theme::TEXT))
            .child(div().w(px(theme::RAIL_WIDTH)).h_full())
            .child(
                div()
                    .flex()
                    .flex_1()
                    .h_full()
                    .rounded_tl(px(theme::RADIUS))
                    .overflow_hidden()
                    .border_1()
                    .border_color(rgb(theme::BORDER))
                    .child(
                        pane(theme::BG_SIDEBAR, "channels")
                            .w(px(theme::SIDEBAR_WIDTH))
                            .child(
                                Button::new("log-out")
                                    .label("Log out")
                                    .on_click(move |_, _, _| backend.send(Command::Logout)),
                            ),
                    )
                    .child(
                        pane(theme::BG_CHAT, "")
                            .flex_1()
                            .child(format!("Signed in as {name}"))
                            .child(status),
                    )
                    .child(pane(theme::BG_MEMBERS, "members").w(px(theme::MEMBERS_WIDTH))),
            )
    }
}

fn pane(bg: u32, label: &'static str) -> gpui_kit::Div {
    div()
        .h_full()
        .flex()
        .flex_col()
        .gap_2()
        .bg(rgb(bg))
        .p_4()
        .text_color(rgb(theme::TEXT_MUTED))
        .child(label)
}

fn connection_label(state: &ConnectionState) -> String {
    match state {
        ConnectionState::Connecting => "Connecting...".into(),
        ConnectionState::Connected => "Connected".into(),
        ConnectionState::Reconnecting { attempt } => format!("Reconnecting (attempt {attempt})..."),
        ConnectionState::AuthFailed => "Discord rejected the token".into(),
        ConnectionState::Closed(reason) => reason.clone(),
    }
}
