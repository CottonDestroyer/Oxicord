//! Agent-facing headless preview support. Kept in the UI crate so a preview
//! binary can build one real view without booting the gateway, login or the
//! rest of the application.

use gpui_kit::{
    AnyView, App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render,
    Styled as _, Window, div, px, rgb,
};
use oxicord_model::InteractionModal;

use crate::{
    backend::Backend,
    interaction_modal::InteractionModalView,
    session::Session,
    settings::Settings,
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
pub const SCENES: &[&str] = &["interaction-modal", "tip", "text"];

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
