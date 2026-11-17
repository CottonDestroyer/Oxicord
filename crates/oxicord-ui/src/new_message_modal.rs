//! The New Message modal opened by the Friends header's New Group DM button.
//! Discord shows a search field, a list of friends and server members, a
//! limit note, and Cancel / Create Message at the bottom.

use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    App, AppContext as _, ClickEvent, Context, Entity, EventEmitter, FontWeight,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, StatefulInteractiveElement as _,
    Styled as _, Window, div, px, rgb,
    component::input::{Input, InputState},
};
use oxicord_media::cdn;
use oxicord_model::{RelationshipKind, Snowflake, User};

use crate::{
    backend::{Backend, Command},
    icons::icon,
    media::Media,
    profile::modal,
    session::Session,
    theme,
};

pub struct CloseNewMessage;

impl EventEmitter<CloseNewMessage> for NewMessageModal {}

/// Live: "Group DMs can have up to 10 members."
const MAX_RECIPIENTS: usize = 10;

pub struct NewMessageModal {
    session: Entity<Session>,
    media: Entity<Media>,
    backend: Backend,
    search: Entity<InputState>,
    picked: Vec<Snowflake>,
    preview_users: Option<Vec<User>>,
    open: bool,
}

impl NewMessageModal {
    pub fn new(
        session: Entity<Session>,
        media: Entity<Media>,
        backend: Backend,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            session,
            media,
            backend,
            search: cx.new(|cx| InputState::new(window, cx).placeholder("Add friends or server members to group DMs.")),
            picked: Vec::new(),
            preview_users: None,
            open: false,
        }
    }

    pub fn opened(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.picked.clear();
        self.search.update(cx, |input, cx| {
            input.set_value("", window, cx);
            input.focus(window, cx);
        });
        self.open = true;
        cx.notify();
    }

    pub fn close(&mut self, cx: &mut Context<Self>) {
        self.open = false;
        cx.emit(CloseNewMessage);
    }

    fn toggle(&mut self, id: Snowflake, cx: &mut Context<Self>) {
        if let Some(at) = self.picked.iter().position(|&user| user == id) {
            self.picked.remove(at);
        } else if self.picked.len() < MAX_RECIPIENTS {
            self.picked.push(id);
        }
        cx.notify();
    }

    #[cfg(feature = "preview")]
    pub fn set_preview_users(&mut self, users: Vec<User>, cx: &mut Context<Self>) {
        self.preview_users = Some(users);
        cx.notify();
    }

    fn candidates(&self, cx: &App) -> Vec<User> {
        let needle = self.search.read(cx).value().trim().to_lowercase();
        let mut users: Vec<User> = self
            .preview_users
            .clone()
            .unwrap_or_else(|| {
                self.session
                    .read(cx)
                    .store
                    .relationships(RelationshipKind::Friend)
                    .into_iter()
                    .cloned()
                    .collect()
            })
            .into_iter()
            .filter(|user| {
                needle.is_empty()
                    || user.display_name().to_lowercase().contains(&needle)
                    || user.username.to_lowercase().contains(&needle)
            })
            .collect();
        users.sort_by_key(|user| user.display_name().to_lowercase());
        users.truncate(MAX_RECIPIENTS.saturating_sub(self.picked.len()) + 8);
        users
    }

    fn create(&mut self, cx: &mut Context<Self>) {
        if self.picked.is_empty() {
            return;
        }
        let picked = std::mem::take(&mut self.picked);
        if picked.len() == 1 {
            self.backend.send(Command::OpenDm(picked[0]));
        } else {
            self.backend.send(Command::OpenGroupDm(picked));
        }
        self.close(cx);
    }

    /// The sheet without the screen overlay, for the headless preview.
    pub fn preview_content(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let users = self.candidates(cx);
        let mut rows = div().id("new-message-results").max_h(px(320.)).overflow_y_scroll().flex().flex_col();
        if users.is_empty() {
            rows = rows.child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .py(px(24.))
                    .gap(px(4.))
                    .child(
                        div()
                            .text_size(px(16.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(theme::TEXT_BRIGHT))
                            .child("No results found"),
                    )
                    .child(
                        div()
                            .text_size(px(14.))
                            .text_color(rgb(theme::TEXT_MUTED))
                            .child("Search your friends or people from your servers."),
                    ),
            );
        }
        for user in &users {
            let id = user.id;
            let on = self.picked.contains(&id);
            let url = cdn::user_avatar(id, user.avatar.as_deref(), 64);
            rows = rows.child(
                div()
                    .id(("new-message-user", id.0))
                    .h(px(44.))
                    .px(px(12.))
                    .flex()
                    .items_center()
                    .gap_3()
                    .rounded(px(theme::RADIUS))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(theme::BG_HOVER)))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.toggle(id, cx)))
                    .child(crate::media::avatar(&self.media, &url, 32., user.display_name(), cx))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_color(rgb(theme::TEXT_BRIGHT))
                            .child(user.display_name().to_owned()),
                    )
                    .child(
                        div()
                            .size(px(20.))
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(theme::RADIUS))
                            .border_1()
                            .border_color(rgb(if on { theme::ACCENT } else { theme::TEXT_FAINT }))
                            .bg(rgb(if on { theme::ACCENT } else { theme::BG_SIDEBAR }))
                            .children(on.then(|| icon("check", 14., theme::TEXT_BRIGHT))),
                    ),
            );
        }

        let ready = !self.picked.is_empty();
        let cancel = div()
            .id("new-message-cancel")
            .h(px(40.))
            .px(px(16.))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(theme::RADIUS))
            .bg(rgb(theme::SECONDARY))
            .border_1()
            .border_color(rgb(theme::BORDER))
            .cursor_pointer()
            .text_size(px(16.))
            .line_height(px(20.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(rgb(theme::TEXT_BRIGHT))
            .hover(|s| s.bg(rgb(theme::SECONDARY_HOVER)))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close(cx)))
            .child("Cancel");
        let create = div()
            .id("new-message-create")
            .h(px(40.))
            .px(px(16.))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(theme::RADIUS))
            .bg(rgb(if ready { theme::ACCENT } else { theme::BG_ACTIVE }))
            .text_size(px(16.))
            .line_height(px(20.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(rgb(theme::TEXT_BRIGHT))
            .when(ready, |button| {
                button
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(theme::ACCENT_HOVER)))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.create(cx)))
            })
            .child("Create Message");

        div()
            .id("new-message-modal")
            .occlude()
            .w(px(480.))
            .max_h(px(720.))
            .flex()
            .flex_col()
            .overflow_hidden()
            .rounded(px(theme::RADIUS_MD))
            .bg(rgb(theme::BG_PANEL))
            .border_1()
            .border_color(rgb(theme::MODAL_BORDER))
            .child(
                div()
                    .flex_none()
                    .px(px(24.))
                    .pt(px(20.))
                    .pb(px(8.))
                    .flex()
                    .items_start()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(4.))
                            .child(
                                div()
                                    .text_size(px(20.))
                                    .line_height(px(24.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(theme::TEXT_BRIGHT))
                                    .child("New Message"),
                            )
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .line_height(px(18.))
                                    .text_color(rgb(theme::TEXT_MUTED))
                                    .child("Group DMs can have up to 10 members."),
                            ),
                    )
                    .child(
                        div()
                            .id("new-message-close")
                            .size(px(24.))
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(theme::RADIUS))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(theme::BG_HOVER)))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close(cx)))
                            .child(icon("x", 20., theme::TEXT_MUTED)),
                    ),
            )
            .child(
                div()
                    .flex_none()
                    .px(px(24.))
                    .py(px(8.))
                    .child(Input::new(&self.search)),
            )
            .child(div().flex_1().px(px(16.)).child(rows))
            .child(
                div()
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_end()
                    .gap(px(8.))
                    .px(px(24.))
                    .pt(px(16.))
                    .pb(px(20.))
                    .child(cancel)
                    .child(create),
            )
            .into_any_element()
    }
}

impl Render for NewMessageModal {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.open {
            return div().into_any_element();
        }
        modal(
            self.preview_content(window, cx),
            Box::new(cx.listener(|this, _: &ClickEvent, _, cx| this.close(cx))),
        )
    }
}
