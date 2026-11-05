//! "Invite friends to a server": a fresh invite link with its expiry and
//! use limit, and a list of conversations to send it to.

use std::collections::HashSet;

use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    ClickEvent, ClipboardItem, Context, Entity, EventEmitter, FontWeight, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled as _, Subscription, Window, div,
    px, rgb,
};
use oxicord_media::cdn;
use oxicord_model::{ChannelType, Snowflake};

use crate::{
    account::Load,
    backend::{Backend, Command},
    media::{Media, avatar},
    profile::modal,
    session::Session,
    theme,
};

/// Seconds an invite lasts, with how Discord words each.
const EXPIRY: [(u32, &str); 7] = [
    (1800, "30 minutes"),
    (3600, "1 hour"),
    (21600, "6 hours"),
    (43200, "12 hours"),
    (86400, "1 day"),
    (604800, "7 days"),
    (0, "Never"),
];

const USES: [(u32, &str); 7] = [
    (0, "No limit"),
    (1, "1 use"),
    (5, "5 uses"),
    (10, "10 uses"),
    (25, "25 uses"),
    (50, "50 uses"),
    (100, "100 uses"),
];

pub struct CloseInvite;

impl EventEmitter<CloseInvite> for InviteDialog {}

pub struct InviteDialog {
    session: Entity<Session>,
    media: Entity<Media>,
    backend: Backend,
    channel: Option<Snowflake>,
    max_age: u32,
    max_uses: u32,
    /// Conversations the link was already sent to.
    sent: HashSet<Snowflake>,
    copied: bool,
    _subscriptions: Vec<Subscription>,
}

impl InviteDialog {
    pub fn new(session: Entity<Session>, media: Entity<Media>, backend: Backend, cx: &mut Context<Self>) -> Self {
        let subscriptions = vec![cx.observe(&session, |_, _, cx| cx.notify()), cx.observe(&media, |_, _, cx| cx.notify())];
        Self {
            session,
            media,
            backend,
            channel: None,
            max_age: 604800,
            max_uses: 0,
            sent: HashSet::new(),
            copied: false,
            _subscriptions: subscriptions,
        }
    }

    /// Opens for `channel` with the defaults the official client starts from.
    pub fn opened(&mut self, channel: Snowflake, cx: &mut Context<Self>) {
        self.channel = Some(channel);
        self.max_age = 604800;
        self.max_uses = 0;
        self.sent.clear();
        self.copied = false;
        self.regenerate(cx);
    }

    fn regenerate(&mut self, cx: &mut Context<Self>) {
        let Some(channel) = self.channel else { return };
        let (backend, age, uses) = (self.backend.clone(), self.max_age, self.max_uses);
        self.copied = false;
        self.session.update(cx, |s, cx| s.request_invite(&backend, channel, age, uses, cx));
    }

    fn link(&self, cx: &gpui_kit::App) -> Option<String> {
        match &self.session.read(cx).created_invite {
            Load::Ready(code) => Some(format!("https://discord.gg/{code}")),
            _ => None,
        }
    }

    fn copy(&mut self, cx: &mut Context<Self>) {
        if let Some(link) = self.link(cx) {
            cx.write_to_clipboard(ClipboardItem::new_string(link));
            self.copied = true;
            cx.notify();
        }
    }

    fn send_to(&mut self, dm: Snowflake, cx: &mut Context<Self>) {
        if let Some(link) = self.link(cx) {
            self.backend.send(Command::SendMessage { channel: dm, guild: None, content: link, reply_to: None, files: Vec::new(), stickers: Vec::new() });
            self.sent.insert(dm);
            cx.notify();
        }
    }

    fn chips(
        &self,
        title: &'static str,
        options: &'static [(u32, &'static str)],
        current: u32,
        set: fn(&mut Self, u32),
        cx: &mut Context<Self>,
    ) -> gpui_kit::AnyElement {
        let mut row = div().flex().flex_wrap().gap_1();
        for (value, label) in options {
            let (value, selected) = (*value, current == *value);
            row = row.child(
                div()
                    .id(SharedString::from(format!("{title}-{value}")))
                    .h(px(26.))
                    .px_2()
                    .flex()
                    .items_center()
                    .rounded(px(6.))
                    .cursor_pointer()
                    .text_size(px(12.))
                    .text_color(rgb(theme::TEXT_BRIGHT))
                    .bg(rgb(if selected { theme::ACCENT } else { theme::BG_ACTIVE }))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        set(this, value);
                        this.regenerate(cx);
                    }))
                    .child(*label),
            );
        }
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(div().text_size(px(11.)).font_weight(FontWeight::BOLD).text_color(rgb(theme::TEXT_MUTED)).child(title.to_uppercase()))
            .child(row)
            .into_any_element()
    }
}

impl Render for InviteDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (guild_name, state, dms) = {
            let s = self.session.read(cx);
            let name = self
                .channel
                .and_then(|c| s.store.channel(c))
                .and_then(|c| c.guild_id)
                .and_then(|g| s.store.guild(g))
                .map(|g| g.name.clone())
                .unwrap_or_default();
            let dms: Vec<_> = s
                .store
                .dm_channels()
                .into_iter()
                .filter(|c| c.kind == ChannelType::Dm)
                .take(30)
                .map(|c| {
                    let url = c
                        .recipients
                        .first()
                        .map(|u| cdn::user_avatar(u.id, u.avatar.as_deref(), 64))
                        .unwrap_or_default();
                    (c.id, s.store.dm_name(c), url)
                })
                .collect();
            (name, s.created_invite.clone(), dms)
        };

        let link_text = match &state {
            Load::Ready(code) => format!("https://discord.gg/{code}"),
            Load::Failed(error) => error.clone(),
            _ => "Making a link...".to_owned(),
        };
        let ready = matches!(state, Load::Ready(_));

        let mut friends = div().id("invite-friends").max_h(px(200.)).overflow_y_scroll().flex().flex_col();
        for (dm, name, url) in dms {
            let done = self.sent.contains(&dm);
            friends = friends.child(
                div()
                    .id(("invite-dm", dm.0))
                    .h(px(40.))
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_3()
                    .rounded(px(4.))
                    .hover(|s| s.bg(rgb(theme::BG_HOVER)))
                    .child(avatar(&self.media, &url, 28., &name, cx))
                    .child(div().flex_1().min_w_0().truncate().text_color(rgb(theme::TEXT_BRIGHT)).child(name))
                    .child(
                        div()
                            .id(("send-invite", dm.0))
                            .h(px(26.))
                            .px_3()
                            .flex()
                            .items_center()
                            .rounded(px(4.))
                            .text_size(px(13.))
                            .text_color(rgb(theme::TEXT_BRIGHT))
                            .bg(rgb(if done { theme::BG_ACTIVE } else { theme::ONLINE }))
                            .when(ready && !done, |b| b.cursor_pointer().hover(|s| s.opacity(0.85)))
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                if !this.sent.contains(&dm) {
                                    this.send_to(dm, cx);
                                }
                            }))
                            .child(if done { "Sent" } else { "Invite" }),
                    ),
            );
        }

        let card = div()
            .id("invite-card")
            .occlude()
            .w(px(460.))
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
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .text_size(px(20.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(theme::TEXT_BRIGHT))
                            .child(format!("Invite friends to {guild_name}")),
                    )
                    .child(div().text_color(rgb(theme::TEXT_MUTED)).child("Send a link to a friend, or copy it.")),
            )
            .child(friends)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .p_1()
                    .rounded(px(6.))
                    .bg(rgb(theme::BG_PANEL))
                    .child(div().flex_1().min_w_0().truncate().px_2().text_color(rgb(theme::TEXT)).child(link_text))
                    .child(
                        div()
                            .id("copy-invite")
                            .h(px(32.))
                            .px_4()
                            .flex()
                            .items_center()
                            .rounded(px(4.))
                            .text_color(rgb(theme::TEXT_BRIGHT))
                            .bg(rgb(if self.copied { theme::ONLINE } else { theme::ACCENT }))
                            .when(ready, |b| b.cursor_pointer().hover(|s| s.opacity(0.85)))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.copy(cx)))
                            .child(if self.copied { "Copied" } else { "Copy" }),
                    ),
            )
            .child(self.chips("Expire after", &EXPIRY, self.max_age, |this, v| this.max_age = v, cx))
            .child(self.chips("Max number of uses", &USES, self.max_uses, |this, v| this.max_uses = v, cx));

        modal(card.into_any_element(), Box::new(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(CloseInvite))))
    }
}
