//! The post list a forum channel shows in place of a message timeline.

use std::collections::HashSet;

use chrono::{DateTime, Utc};
use gpui_kit::{
    AnyElement, AppContext as _, ClickEvent, Context, Entity, FontWeight, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _,
    Subscription, Window, div, px, rgb,
    component::input::{Input, InputState, Textarea, TextareaState},
};
use oxicord_model::{Channel, Message, Permissions, Snowflake};

use crate::{
    account::Load,
    backend::{Backend, Command},
    icons::icon,
    media::Media,
    session::Session,
    theme,
};

/// Longest stretch of a post's first message shown in the list.
const PREVIEW_CHARS: usize = 220;

pub struct ForumView {
    session: Entity<Session>,
    backend: Backend,
    forum: Option<Snowflake>,
    /// Tags the list is narrowed to. Empty shows everything.
    filter: HashSet<Snowflake>,
    composing: bool,
    title: Entity<InputState>,
    body: Entity<TextareaState>,
    new_tags: HashSet<Snowflake>,
    _subscriptions: Vec<Subscription>,
}

impl ForumView {
    pub fn new(
        session: Entity<Session>,
        _media: Entity<Media>,
        backend: Backend,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let title = cx.new(|cx| InputState::new(window, cx).placeholder("Post title"));
        let body = cx.new(|cx| {
            TextareaState::new(window, cx)
                .auto_grow(3, 10)
                .placeholder("Write the first message")
        });
        let subscriptions = vec![cx.observe(&session, |this, _, cx| {
            this.fetch_previews(cx);
            cx.notify();
        })];
        Self {
            session,
            backend,
            forum: None,
            filter: HashSet::new(),
            composing: false,
            title,
            body,
            new_tags: HashSet::new(),
            _subscriptions: subscriptions,
        }
    }

    /// Points the view at a forum and loads its older posts.
    pub fn set_forum(&mut self, forum: Option<Snowflake>, cx: &mut Context<Self>) {
        if self.forum == forum {
            return;
        }
        self.forum = forum;
        self.filter.clear();
        self.composing = false;
        self.new_tags.clear();
        if let Some(forum) = forum {
            let backend = self.backend.clone();
            self.session
                .update(cx, |session, cx| session.request_threads(&backend, forum, cx));
        }
        self.fetch_previews(cx);
        cx.notify();
    }

    /// Asks for the opening message of every post on screen that we lack.
    fn fetch_previews(&mut self, cx: &mut Context<Self>) {
        let Some(forum) = self.forum else { return };
        let ids: Vec<Snowflake> = self
            .posts(cx)
            .into_iter()
            .map(|p| p.id)
            .collect();
        let backend = self.backend.clone();
        self.session
            .update(cx, |session, _| session.request_previews(&backend, forum, &ids));
    }

    /// Open posts plus the archived ones loaded so far, pinned first and then
    /// by latest activity.
    fn posts(&self, cx: &gpui_kit::App) -> Vec<Channel> {
        let Some(forum) = self.forum else { return Vec::new() };
        let session = self.session.read(cx);
        let mut posts: Vec<Channel> = session.store.open_threads(forum).into_iter().cloned().collect();
        if let Some(Load::Ready(page)) = session.archived_threads.get(&forum) {
            for thread in &page.threads {
                if !posts.iter().any(|p| p.id == thread.id) {
                    posts.push(thread.clone());
                }
            }
        }
        if !self.filter.is_empty() {
            posts.retain(|p| p.applied_tags.iter().any(|t| self.filter.contains(t)));
        }
        posts.sort_by(|a, b| {
            b.pinned_post()
                .cmp(&a.pinned_post())
                .then(b.last_message_id.cmp(&a.last_message_id))
                .then(b.id.cmp(&a.id))
        });
        posts
    }

    fn can_post(&self, cx: &gpui_kit::App) -> bool {
        let Some(forum) = self.forum else { return false };
        let store = &self.session.read(cx).store;
        store
            .channel(forum)
            .is_some_and(|c| store.permissions_in(c).contains(Permissions::SEND_MESSAGES))
    }

    fn publish(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(forum) = self.forum else { return };
        let title = self.title.read(cx).value().trim().to_owned();
        let content = self.body.read(cx).value().trim().to_owned();
        if title.is_empty() || content.is_empty() {
            return;
        }
        let tags: Vec<Snowflake> = self.new_tags.iter().copied().collect();
        self.backend.send(Command::CreateForumPost { forum, title, content, tags });
        self.composing = false;
        self.new_tags.clear();
        self.title.update(cx, |i, cx| i.set_value("", window, cx));
        self.body.update(cx, |i, cx| i.set_value("", window, cx));
        cx.notify();
    }

    fn tag_chip(
        &self,
        id: Snowflake,
        name: &str,
        selected: bool,
        on_click: impl Fn(&ClickEvent, &mut Window, &mut gpui_kit::App) + 'static,
    ) -> AnyElement {
        div()
            .id(("tag", id.0))
            .h(px(24.))
            .px_2()
            .flex()
            .items_center()
            .rounded(px(theme::RADIUS))
            .cursor_pointer()
            .text_size(px(12.))
            .text_color(rgb(if selected { theme::TEXT_BRIGHT } else { theme::TEXT_MUTED }))
            .bg(rgb(if selected { theme::ACCENT } else { theme::BG_ACTIVE }))
            .hover(|s| s.opacity(0.85))
            .on_click(on_click)
            .child(name.to_owned())
            .into_any_element()
    }

    fn render_post(&mut self, post: &Channel, forum_tags: &[(Snowflake, String)], cx: &mut Context<Self>) -> AnyElement {
        let id = post.id;
        let (preview, unread) = {
            let session = self.session.read(cx);
            (session.previews.get(&id).cloned(), session.store.unread(id).unread)
        };

        let author = preview
            .as_ref()
            .map(|m| m.author.display_name().to_owned())
            .unwrap_or_default();
        let text = preview.as_ref().map(snippet).unwrap_or_default();
        let replies = post.message_count.unwrap_or(0);
        let when = post.last_message_id.map(|m| ago(m.datetime(), Utc::now())).unwrap_or_default();

        let tags: Vec<AnyElement> = post
            .applied_tags
            .iter()
            .filter_map(|t| forum_tags.iter().find(|(id, _)| id == t))
            .map(|(_, name)| {
                div()
                    .px_2()
                    .h(px(20.))
                    .flex()
                    .items_center()
                    .rounded(px(5.))
                    .bg(rgb(theme::BG_ACTIVE))
                    .text_size(px(11.))
                    .text_color(rgb(theme::TEXT_MUTED))
                    .child(name.clone())
                    .into_any_element()
            })
            .collect();

        div()
            .id(("post", id.0))
            .mx_4()
            .p_3()
            .flex()
            .flex_col()
            .gap_1()
            .rounded(px(8.))
            .bg(rgb(theme::BG_SIDEBAR))
            .border_1()
            .border_color(rgb(theme::BORDER))
            .cursor_pointer()
            .hover(|s| s.bg(rgb(theme::BG_HOVER)))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.session.update(cx, |s, cx| s.jump_to(id, cx));
            }))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .children(unread.then(|| div().size(px(8.)).flex_none().rounded_full().bg(rgb(theme::TEXT_BRIGHT))))
                    .children(post.pinned_post().then(|| icon("pin", 14., theme::TEXT_MUTED)))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(theme::TEXT_BRIGHT))
                            .child(post.name.clone().unwrap_or_default()),
                    ),
            )
            .children((!tags.is_empty()).then(|| div().flex().flex_wrap().gap_1().children(tags)))
            .children((!text.is_empty()).then(|| {
                div()
                    .text_size(px(13.))
                    .text_color(rgb(theme::TEXT))
                    .child(text)
            }))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .text_size(px(12.))
                    .text_color(rgb(theme::TEXT_MUTED))
                    .child(author)
                    .child(format!("{replies} repl{}", if replies == 1 { "y" } else { "ies" }))
                    .child(when),
            )
            .into_any_element()
    }

    fn render_compose(&mut self, forum_tags: &[(Snowflake, String)], cx: &mut Context<Self>) -> AnyElement {
        let chips: Vec<AnyElement> = forum_tags
            .iter()
            .map(|(id, name)| {
                let id = *id;
                let selected = self.new_tags.contains(&id);
                self.tag_chip(id, name, selected, cx.listener(move |this, _: &ClickEvent, _, cx| {
                    if !this.new_tags.remove(&id) {
                        this.new_tags.insert(id);
                    }
                    cx.notify();
                }))
            })
            .collect();

        div()
            .mx_4()
            .mt_3()
            .p_3()
            .flex()
            .flex_col()
            .gap_2()
            .rounded(px(8.))
            .border_1()
            .border_color(rgb(theme::BORDER))
            .bg(rgb(theme::BG_SIDEBAR))
            .child(Input::new(&self.title))
            .child(Textarea::new(&self.body))
            .children((!chips.is_empty()).then(|| div().flex().flex_wrap().gap_1().children(chips)))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(small_button("post-publish", "Post", true, cx.listener(
                        |this, _: &ClickEvent, window, cx| this.publish(window, cx),
                    )))
                    .child(small_button("post-cancel", "Cancel", false, cx.listener(
                        |this, _: &ClickEvent, _, cx| {
                            this.composing = false;
                            cx.notify();
                        },
                    ))),
            )
            .into_any_element()
    }
}

impl Render for ForumView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (name, topic, tags) = self
            .forum
            .and_then(|f| {
                let store = &self.session.read(cx).store;
                let c = store.channel(f)?;
                Some((c.name.clone().unwrap_or_default(), c.topic.clone(), store.forum_tags(f)))
            })
            .unwrap_or_default();
        let posts = self.posts(cx);
        let loading = self
            .forum
            .and_then(|f| self.session.read(cx).archived_threads.get(&f).cloned())
            .is_some_and(|l| matches!(l, Load::Loading));
        let can_post = self.can_post(cx);

        let filter_row: Vec<AnyElement> = tags
            .iter()
            .map(|(id, tag)| {
                let id = *id;
                let selected = self.filter.contains(&id);
                self.tag_chip(id, tag, selected, cx.listener(move |this, _: &ClickEvent, _, cx| {
                    if !this.filter.remove(&id) {
                        this.filter.insert(id);
                    }
                    cx.notify();
                }))
            })
            .collect();

        let mut list = div().id("forum-posts").flex_1().min_h_0().overflow_y_scroll().flex().flex_col().gap_2().py_3();
        if self.composing {
            list = list.child(self.render_compose(&tags, cx));
        }
        if posts.is_empty() {
            list = list.child(
                div()
                    .p_6()
                    .text_color(rgb(theme::TEXT_MUTED))
                    .child(if loading { "Loading posts..." } else { "No posts yet." }),
            );
        }
        for post in &posts {
            list = list.child(self.render_post(post, &tags, cx));
        }

        div()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .flex_col()
            .bg(rgb(theme::BG_CHAT))
            .child(
                div()
                    .h(px(48.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_4()
                    .border_b_1()
                    .border_color(rgb(theme::BORDER))
                    .child(icon("messages-square", 20., theme::TEXT_MUTED))
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(theme::TEXT_BRIGHT))
                            .child(name),
                    )
                    .children(topic.filter(|t| !t.is_empty()).map(|t| {
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .ml_2()
                            .pl_3()
                            .border_l_1()
                            .border_color(rgb(theme::BORDER))
                            .text_size(px(13.))
                            .text_color(rgb(theme::TEXT_MUTED))
                            .child(t)
                    }))
                    .child(div().flex_1())
                    .children(can_post.then(|| {
                        small_button("new-post", "New Post", true, cx.listener(
                            |this, _: &ClickEvent, _, cx| {
                                this.composing = !this.composing;
                                cx.notify();
                            },
                        ))
                    })),
            )
            .children((!filter_row.is_empty()).then(|| {
                div().flex().flex_wrap().gap_1().px_4().pt_3().children(filter_row)
            }))
            .child(list)
    }
}

fn small_button(
    id: &'static str,
    label: &'static str,
    primary: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut gpui_kit::App) + 'static,
) -> AnyElement {
    div()
        .id(id)
        .h(px(28.))
        .px_3()
        .flex()
        .items_center()
        .rounded(px(theme::RADIUS))
        .cursor_pointer()
        .text_color(rgb(theme::TEXT_BRIGHT))
        .bg(rgb(if primary { theme::ACCENT } else { theme::BG_ACTIVE }))
        .hover(|s| s.opacity(0.85))
        .on_click(on_click)
        .child(label)
        .into_any_element()
}

/// The start of a post's first message, as plain text.
fn snippet(message: &Message) -> String {
    let text = message.content.split_whitespace().collect::<Vec<_>>().join(" ");
    match text.char_indices().nth(PREVIEW_CHARS) {
        Some((end, _)) => format!("{}...", &text[..end]),
        None => text,
    }
}

/// "5 minutes ago" in the short form lists use.
fn ago(then: DateTime<Utc>, now: DateTime<Utc>) -> String {
    let seconds = (now - then).num_seconds().max(0);
    match seconds {
        0..=59 => "just now".to_owned(),
        60..=3599 => format!("{}m ago", seconds / 60),
        3600..=86_399 => format!("{}h ago", seconds / 3600),
        86_400..=2_591_999 => format!("{}d ago", seconds / 86_400),
        _ => then.format("%d/%m/%Y").to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(s: &str) -> DateTime<Utc> {
        s.parse().unwrap()
    }

    #[test]
    fn ages_use_the_largest_sensible_unit() {
        let now = at("2024-05-10T12:00:00Z");
        assert_eq!(ago(at("2024-05-10T11:59:50Z"), now), "just now");
        assert_eq!(ago(at("2024-05-10T11:15:00Z"), now), "45m ago");
        assert_eq!(ago(at("2024-05-10T07:00:00Z"), now), "5h ago");
        assert_eq!(ago(at("2024-05-07T12:00:00Z"), now), "3d ago");
        assert_eq!(ago(at("2024-01-01T12:00:00Z"), now), "01/01/2024");
        // A clock a little ahead of the server never reads as the future.
        assert_eq!(ago(at("2024-05-10T12:00:05Z"), now), "just now");
    }
}
