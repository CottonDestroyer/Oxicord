use gpui_kit::{
    AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Task, Window, div,
};
use oxicord_gateway::{ConnectionState, GatewayEvent};

use crate::{
    backend::{Backend, Command, Event},
    login::LoginView,
    main_view::MainView,
    session::Session,
};

enum Screen {
    Login(Entity<LoginView>),
    Main(Entity<MainView>),
}

/// Root of the window. Owns the session and decides whether to show the
/// login form or the client.
pub struct AppView {
    backend: Backend,
    session: Entity<Session>,
    screen: Screen,
    /// Shown on the next login screen, for example after a revoked token.
    notice: Option<SharedString>,
    _pump: Task<()>,
}

impl AppView {
    /// `saved_token` signs in straight away when we have one stored.
    pub fn new(
        backend: Backend,
        saved_token: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let session = cx.new(|_| Session::new());
        let login = Self::login_view(&backend, None, window, cx);
        if let Some(token) = saved_token {
            login.update(cx, |login, cx| login.sign_in_with(token, cx));
        }

        let events = backend.events.clone();
        let pump = cx.spawn_in(window, async move |this, cx| {
            while let Ok(event) = events.recv().await {
                let handled = this.update_in(cx, |this, window, cx| this.on_event(event, window, cx));
                if handled.is_err() {
                    break;
                }
            }
        });

        Self {
            backend,
            session,
            screen: Screen::Login(login),
            notice: None,
            _pump: pump,
        }
    }

    fn login_view(
        backend: &Backend,
        notice: Option<SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<LoginView> {
        let backend = backend.clone();
        cx.new(|cx| LoginView::new(backend, notice, window, cx))
    }

    fn on_event(&mut self, event: Event, window: &mut Window, cx: &mut Context<Self>) {
        match event {
            Event::Authenticated(_) => {
                let (session, backend) = (self.session.clone(), self.backend.clone());
                let main = cx.new(|cx| MainView::new(session, backend, cx));
                self.screen = Screen::Main(main);
            }
            Event::LoginFailed(message) => {
                if let Screen::Login(login) = &self.screen {
                    login.update(cx, |login, cx| login.fail(message, cx));
                }
            }
            Event::LoggedOut => {
                self.session = cx.new(|_| Session::new());
                let notice = self.notice.take();
                self.screen = Screen::Login(Self::login_view(&self.backend, notice, window, cx));
            }
            Event::Gateway(event) => {
                if matches!(event, GatewayEvent::State(ConnectionState::AuthFailed)) {
                    self.notice = Some("Discord no longer accepts that token. Sign in again.".into());
                    self.backend.send(Command::Logout);
                }
                self.session.update(cx, |session, cx| session.handle(event, cx));
            }
        }
        cx.notify();
    }
}

impl Render for AppView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        match &self.screen {
            Screen::Login(view) => div().size_full().child(view.clone()),
            Screen::Main(view) => div().size_full().child(view.clone()),
        }
    }
}
