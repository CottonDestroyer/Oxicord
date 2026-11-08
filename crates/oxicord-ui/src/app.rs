use gpui_kit::{
    AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Task, Window, div, px, rgb,
};
use oxicord_gateway::{ClientStatus, ConnectionState, GatewayEvent};
use oxicord_model::gateway::DispatchEvent;

use crate::{
    backend::{Backend, Command, Event},
    login::LoginView,
    main_view::MainView,
    media::Media,
    session::Session,
    settings::Settings,
    theme, title_bar,
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
    media: Entity<Media>,
    screen: Screen,
    /// Shown on the next login screen, for example after a revoked token.
    notice: Option<SharedString>,
    _pump: Task<()>,
    /// Debug builds poll a command file so a script can steer the app.
    #[cfg(debug_assertions)]
    _dev: Task<()>,
}

impl AppView {
    /// `saved_token` signs in straight away when we have one stored.
    pub fn new(
        backend: Backend,
        saved_token: Option<String>,
        settings: Settings,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let session = cx.new(|_| Session::new(settings));
        let media = cx.new(|_| Media::new(backend.clone()));
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

        #[cfg(debug_assertions)]
        let dev = cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor().timer(std::time::Duration::from_millis(500)).await;
                let commands = crate::dev_control::take();
                if commands.is_empty() {
                    continue;
                }
                let Ok(main) = this.update(cx, |this, _| match &this.screen {
                    Screen::Main(main) => Some(main.clone()),
                    Screen::Login(_) => None,
                }) else {
                    break;
                };
                let Some(main) = main else { continue };
                for command in commands {
                    if main.update_in(cx, |main, window, cx| main.dev_command(command, window, cx)).is_err() {
                        return;
                    }
                }
            }
        });

        Self {
            backend,
            session,
            media,
            screen: Screen::Login(login),
            notice: None,
            _pump: pump,
            #[cfg(debug_assertions)]
            _dev: dev,
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

    /// Raises a toast for a message that arrived while the window was in the
    /// background. Do not disturb stays quiet, as it does elsewhere.
    fn notify(&self, message: &oxicord_model::Message, cx: &Context<Self>) {
        let session = self.session.read(cx);
        if !session.settings.desktop_notifications
            || session.status == ClientStatus::Dnd
            || !session.store.is_notable(message) {
            return;
        }
        let (title, body) = crate::notify::compose(&session.store, message);
        self.backend.send(Command::Notify { title, body });
    }

    fn on_event(&mut self, event: Event, window: &mut Window, cx: &mut Context<Self>) {
        match event {
            Event::Authenticated(_) => {
                let (session, media, backend) =
                    (self.session.clone(), self.media.clone(), self.backend.clone());
                let main = cx.new(|cx| MainView::new(session, media, backend, window, cx));
                self.screen = Screen::Main(main);
            }
            Event::LoginFailed(message) => {
                if let Screen::Login(login) = &self.screen {
                    login.update(cx, |login, cx| login.fail(message, cx));
                }
            }
            Event::LoggedOut => {
                let settings = self.session.read(cx).settings.clone();
                self.session = cx.new(|_| Session::new(settings));
                let notice = self.notice.take();
                self.screen = Screen::Login(Self::login_view(&self.backend, notice, window, cx));
            }
            Event::History {
                channel,
                result,
                requested,
            } => {
                self.session.update(cx, |session, cx| {
                    session.apply_history(channel, result, requested, cx)
                });
            }
            Event::Threads { channel, result, .. } => {
                self.session.update(cx, |session, cx| session.apply_threads(channel, result, cx));
            }
            Event::Around { channel, message, result } => {
                self.session
                    .update(cx, |session, cx| session.apply_around(channel, message, result, cx));
            }
            Event::Gifs { query, result } => {
                self.session.update(cx, |session, cx| session.apply_gifs(query, result, cx));
            }
            Event::Reactors { message, key, count, names } => {
                self.session.update(cx, |session, _| {
                    session.reactors.insert((message, key, count), names);
                });
            }
            Event::OwnNick { guild, nick } => {
                self.session.update(cx, |session, cx| {
                    session.own_nicks.insert(guild, nick);
                    cx.notify();
                });
            }
            Event::Users(users) => {
                self.session.update(cx, |session, cx| session.apply_users(users, cx));
            }
            Event::Commands { channel, result } => {
                self.session.update(cx, |session, cx| session.apply_commands(channel, result, cx));
            }
            Event::InvitePreview(result) => {
                self.session.update(cx, |session, cx| session.apply_invite_preview(result, cx));
            }
            Event::InviteCreated(result) => {
                self.session.update(cx, |session, cx| session.apply_created_invite(result, cx));
            }
            Event::InviteJoined(result) => {
                self.session.update(cx, |session, cx| session.apply_invite_joined(result, cx));
            }
            Event::FriendRequest { username, result } => {
                self.session
                    .update(cx, |session, cx| session.apply_friend_request(username, result, cx));
            }
            Event::ForumPreviews(messages) => {
                self.session.update(cx, |session, cx| session.apply_previews(messages, cx));
            }
            Event::ThreadCreated(result) => {
                self.session.update(cx, |session, cx| session.apply_new_thread(result, cx));
            }
            Event::Mentions(result) => {
                self.session.update(cx, |session, cx| session.apply_mentions(result, cx));
            }
            Event::SavedMessages(result) => {
                self.session.update(cx, |session, cx| session.apply_saved_messages(result, cx));
            }
            Event::ScheduledMessages(result) => {
                self.session.update(cx, |session, cx| session.apply_scheduled_messages(result, cx));
            }
            Event::Pins { channel, result } => {
                self.session.update(cx, |session, cx| session.apply_pins(channel, result, cx));
            }
            Event::Search { raw, result, .. } => {
                self.session.update(cx, |session, cx| session.apply_search(raw, result, cx));
            }
            Event::Admin(event) => {
                self.session.update(cx, |session, cx| session.apply_admin(event, cx));
            }
            Event::Account(event) => {
                self.session.update(cx, |session, cx| session.apply_account(event, cx));
            }
            Event::Profile { user, result } => {
                self.session
                    .update(cx, |session, cx| session.apply_profile(user, result, cx));
            }
            Event::PollVoters { message, answer: _, users } => {
                self.session.update(cx, |session, cx| {
                    session.store.add_users(users);
                    cx.emit(oxicord_store::Changes::full());
                });
                let _ = message;
            }
            Event::DmOpened(result) => {
                self.session.update(cx, |session, cx| session.apply_dm(result, cx));
            }
            Event::Voice(notice) => {
                self.session.update(cx, |session, cx| session.apply_voice(notice, cx));
            }
            Event::Notice(text) => {
                self.session
                    .update(cx, |session, cx| session.set_notice(Some(text), cx));
            }
            Event::Image { url, bytes } => {
                self.media.update(cx, |media, cx| media.on_image(url, bytes, cx));
            }
            Event::Gateway(event) => {
                if matches!(event, GatewayEvent::State(ConnectionState::AuthFailed)) {
                    self.notice = Some("Discord no longer accepts that token. Sign in again.".into());
                    self.backend.send(Command::Logout);
                }
                let incoming = (!window.is_window_active())
                    .then(|| match &event {
                        GatewayEvent::Dispatch(DispatchEvent::MessageCreate(m)) => Some((**m).clone()),
                        _ => None,
                    })
                    .flatten();
                let backend = self.backend.clone();
                self.session.update(cx, |session, cx| session.handle(&backend, event, cx));
                if let Some(message) = incoming {
                    self.notify(&message, cx);
                }
            }
        }
        cx.notify();
    }
}

impl Render for AppView {
    fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        match &self.screen {
            Screen::Login(view) => div()
                .size_full()
                .font_family(theme::font_family())
                .relative()
                .pt(px(title_bar::HEIGHT))
                .bg(rgb(theme::BG_RAIL))
                .child(view.clone())
                .child(title_bar::title_bar("Oxicord".into(), window)),
            // The signed in frame draws its own, naming what is open.
            Screen::Main(view) => div()
                .size_full()
                .font_family(theme::font_family())
                .child(view.clone()),
        }
    }
}
