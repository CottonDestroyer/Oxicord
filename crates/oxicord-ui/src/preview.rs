//! Agent-facing headless preview support. Kept in the UI crate so a preview
//! binary can build one real view without booting the gateway, login or the
//! rest of the application.

use gpui_kit::{
    AnyView, App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render,
    Styled as _, Window, div, px, rgb,
};
use oxicord_model::{InteractionModal, User};

use crate::{
    backend::Backend,
    interaction_modal::InteractionModalView,
    media::Media,
    new_message_modal::NewMessageModal,
    session::Session,
    settings::Settings,
    settings_view::SettingsView,
    theme,
    tip::Tip,
};

/// The window root: a dark canvas with one scene on it.
pub struct PreviewRoot(pub AnyView);

impl Render for PreviewRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().bg(rgb(theme::BG_CHAT)).child(self.0.clone())
    }
}

/// The scenes an agent can ask for by `--scene`.
pub const SCENES: &[&str] = &[
    "interaction-modal",
    "new-message",
    "settings",
    "settings-privacy",
    "settings-messaging",
    "settings-notifications",
    "settings-voice",
    "settings-appearance",
    "settings-accessibility",
    "settings-system",
    "settings-language",
    "settings-activity",
    "settings-apps",
    "settings-developer",
    "tip",
    "text",
];

struct SettingsSheet(Entity<SettingsView>);

impl Render for SettingsSheet {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let sheet = self.0.update(cx, |view, cx| view.preview_content(window, cx));
        div().size_full().bg(rgb(theme::BG_CHAT)).child(sheet)
    }
}

struct NewMessageSheet(Entity<NewMessageModal>);

impl Render for NewMessageSheet {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let sheet = self.0.update(cx, |view, cx| view.preview_content(window, cx));
        div().size_full().bg(rgb(theme::BG_CHAT)).child(sheet)
    }
}

struct ModalSheet(Entity<crate::interaction_modal::InteractionModalView>);

impl Render for ModalSheet {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let sheet = self.0.update(cx, |view, cx| view.preview_content(cx));
        div().size_full().bg(rgb(theme::BG_CHAT)).child(sheet)
    }
}

struct HelloText;

impl Render for HelloText {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .p(px(20.))
            .text_size(px(24.))
            .text_color(rgb(0xffff_ffff))
            .child("Hello preview")
    }
}

/// An in-memory backend: commands go nowhere, which is fine for layout.
fn backend() -> Backend {
    let (commands, _) = async_channel::unbounded();
    let (_, events) = async_channel::unbounded();
    Backend { commands, events }
}

/// A session with fixture account data, so settings pages render their real
/// rows in headless previews instead of the loading gate.
fn fixture_session(cx: &mut App) -> Entity<Session> {
    let session = cx.new(|_| Session::new(Settings::default()));
    session.update(cx, |session, _| {
        session.account.settings = crate::account::Load::Ready(Default::default());
        session.account.details = crate::account::Load::Ready(oxicord_model::AccountDetails {
            email: Some("agent@example.com".into()),
            phone: Some("+1 555 0100".into()),
            verified: true,
            ..Default::default()
        });
        session.account.sessions = crate::account::Load::Ready(vec![]);
    });
    session
}

fn root(view: AnyView, cx: &mut App) -> Entity<PreviewRoot> {
    cx.new(|_| PreviewRoot(view))
}

/// Builds a scene as the window root. The caller owns the headless window.
pub fn open_scene(name: &str, window: &mut Window, cx: &mut App) -> Entity<PreviewRoot> {
    match name {
        "interaction-modal" => {
            let text = std::fs::read_to_string("fixtures/interaction-modal.json")
                .expect("fixtures/interaction-modal.json must exist");
            let modal: InteractionModal = serde_json::from_str(&text).expect("a valid modal fixture");
            let session: Entity<Session> = cx.new(|_| Session::new(Settings::default()));
            let view = cx.new(|cx| InteractionModalView::new(session, backend(), cx));
            view.update(cx, |view, cx| view.opened(modal, window, cx));
            root(cx.new(|_| ModalSheet(view)).into(), cx)
        }
        "new-message" => {
            let session: Entity<Session> = cx.new(|_| Session::new(Settings::default()));
            let media: Entity<Media> = cx.new(|_| Media::new(backend()));
            let view = cx.new(|cx| NewMessageModal::new(session, media, backend(), window, cx));
            view.update(cx, |view, cx| {
                view.set_preview_users(
                    [
                        r#"{"id":"10","username":"alex","global_name":"Alex Rivera"}"#,
                        r#"{"id":"11","username":"sam","global_name":"Sam Lee"}"#,
                        r#"{"id":"12","username":"jordan","global_name":"Jordan Kim"}"#,
                    ]
                    .into_iter()
                    .map(|json| serde_json::from_str::<User>(json).expect("sample user"))
                    .collect(),
                    cx,
                );
                view.opened(window, cx);
            });
            root(cx.new(|_| NewMessageSheet(view)).into(), cx)
        }
        "settings" => {
            let session = fixture_session(cx);
            let media: Entity<Media> = cx.new(|_| Media::new(backend()));
            let view = cx.new(|cx| SettingsView::new(session, media, backend(), window, cx));
            view.update(cx, |view, cx| view.opened(cx));
            root(cx.new(|_| SettingsSheet(view)).into(), cx)
        }
        "settings-privacy" => {
            let session = fixture_session(cx);
            let media: Entity<Media> = cx.new(|_| Media::new(backend()));
            let view = cx.new(|cx| SettingsView::new(session, media, backend(), window, cx));
            view.update(cx, |view, cx| view.open_data_privacy(cx));
            root(cx.new(|_| SettingsSheet(view)).into(), cx)
        }
        "settings-messaging" => {
            let session = fixture_session(cx);
            let media: Entity<Media> = cx.new(|_| Media::new(backend()));
            let view = cx.new(|cx| SettingsView::new(session, media, backend(), window, cx));
            view.update(cx, |view, cx| view.open_messaging(cx));
            root(cx.new(|_| SettingsSheet(view)).into(), cx)
        }
        "settings-notifications" => {
            let session = fixture_session(cx);
            let media: Entity<Media> = cx.new(|_| Media::new(backend()));
            let view = cx.new(|cx| SettingsView::new(session, media, backend(), window, cx));
            view.update(cx, |view, cx| view.open_notifications(cx));
            root(cx.new(|_| SettingsSheet(view)).into(), cx)
        }
        "settings-voice" => {
            let session = fixture_session(cx);
            let media: Entity<Media> = cx.new(|_| Media::new(backend()));
            let view = cx.new(|cx| SettingsView::new(session, media, backend(), window, cx));
            view.update(cx, |view, cx| view.open_voice(cx));
            root(cx.new(|_| SettingsSheet(view)).into(), cx)
        }
        "settings-appearance" => {
            let session = fixture_session(cx);
            let media: Entity<Media> = cx.new(|_| Media::new(backend()));
            let view = cx.new(|cx| SettingsView::new(session, media, backend(), window, cx));
            view.update(cx, |view, cx| view.open_appearance(cx));
            root(cx.new(|_| SettingsSheet(view)).into(), cx)
        }
        "settings-accessibility" => {
            let session = fixture_session(cx);
            let media: Entity<Media> = cx.new(|_| Media::new(backend()));
            let view = cx.new(|cx| SettingsView::new(session, media, backend(), window, cx));
            view.update(cx, |view, cx| view.open_accessibility(cx));
            root(cx.new(|_| SettingsSheet(view)).into(), cx)
        }
        "settings-system" => {
            let session = fixture_session(cx);
            let media: Entity<Media> = cx.new(|_| Media::new(backend()));
            let view = cx.new(|cx| SettingsView::new(session, media, backend(), window, cx));
            view.update(cx, |view, cx| view.open_system(cx));
            root(cx.new(|_| SettingsSheet(view)).into(), cx)
        }
        "settings-language" => {
            let session = fixture_session(cx);
            let media: Entity<Media> = cx.new(|_| Media::new(backend()));
            let view = cx.new(|cx| SettingsView::new(session, media, backend(), window, cx));
            view.update(cx, |view, cx| view.open_language_time(cx));
            root(cx.new(|_| SettingsSheet(view)).into(), cx)
        }
        "settings-activity" => {
            let session = fixture_session(cx);
            let media: Entity<Media> = cx.new(|_| Media::new(backend()));
            let view = cx.new(|cx| SettingsView::new(session, media, backend(), window, cx));
            view.update(cx, |view, cx| view.open_activity_privacy(cx));
            root(cx.new(|_| SettingsSheet(view)).into(), cx)
        }
        "settings-apps" => {
            let session = fixture_session(cx);
            let media: Entity<Media> = cx.new(|_| Media::new(backend()));
            let view = cx.new(|cx| SettingsView::new(session, media, backend(), window, cx));
            view.update(cx, |view, cx| view.open_connected_apps(cx));
            root(cx.new(|_| SettingsSheet(view)).into(), cx)
        }
        "settings-developer" => {
            let session = fixture_session(cx);
            let media: Entity<Media> = cx.new(|_| Media::new(backend()));
            let view = cx.new(|cx| SettingsView::new(session, media, backend(), window, cx));
            view.update(cx, |view, cx| view.open_developer(cx));
            root(cx.new(|_| SettingsSheet(view)).into(), cx)
        }
        "tip" => {
            let view: Entity<Tip> = cx.new(|_| Tip("Agent preview ready".into()));
            root(view.into(), cx)
        }
        "text" => {
            let view = cx.new(|_| HelloText);
            root(view.into(), cx)
        }
        other => panic!("unknown scene {other:?}; try {}", SCENES.join(", ")),
    }
}
