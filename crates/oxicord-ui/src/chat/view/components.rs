//! Draws a message's buttons, menus and layout components, and sends the
//! clicks back to the bot that owns them.

use gpui_kit::{
    AnyElement, Context, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _,
    SharedString, StatefulInteractiveElement as _, Styled as _, Window, div, px, relative, rgb,
};
use oxicord_model::{ButtonStyle, Component, MediaRef, Message, Snowflake, component::owner};

use super::ChatView;
use crate::{
    backend::Command,
    chat::attachments::{fit, picture, resized},
    icons::icon,
    theme,
};

const MAX_PICTURE: f32 = 360.;
const THUMBNAIL: f32 = 96.;
const GREEN: u32 = 0x248046;
const RED: u32 = 0xda373c;
const GREY: u32 = 0x4e5058;

impl ChatView {
    /// All of a message's components, or nothing when it has none.
    pub(super) fn render_components(
        &mut self,
        message: &Message,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if message.components.is_empty() {
            return None;
        }
        let mut counter = 0u32;
        let mut column = div().mt_1().flex().flex_col().gap(px(6.));
        for component in &message.components {
            column = column.child(self.component(message, component, &mut counter, window, cx));
        }
        Some(column.into_any_element())
    }

    fn component(
        &mut self,
        message: &Message,
        component: &Component,
        counter: &mut u32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        *counter += 1;
        let salt = u64::from(*counter);
        let key = SharedString::from(format!("component-{}-{salt}", message.id));

        match component {
            Component::ActionRow(items) => {
                let mut row = div().flex().flex_wrap().items_start().gap_2();
                for item in items {
                    row = row.child(self.component(message, item, counter, window, cx));
                }
                row.into_any_element()
            }
            Component::Button { style, label, custom_id, url, emoji, disabled } => {
                let fill = match style {
                    ButtonStyle::Primary => theme::ACCENT,
                    ButtonStyle::Secondary | ButtonStyle::Link => GREY,
                    ButtonStyle::Success => GREEN,
                    ButtonStyle::Danger => RED,
                };
                // Custom emoji are pictures, which a button doesn't draw.
                let unicode = emoji.as_ref().filter(|e| e.id.is_none()).and_then(|e| e.name.clone());
                let caption = match (unicode, label) {
                    (Some(e), Some(l)) => format!("{e} {l}"),
                    (Some(e), None) => e,
                    (None, Some(l)) => l.clone(),
                    (None, None) => String::new(),
                };
                let button = div()
                    .id(key)
                    .h(px(32.))
                    .px_3()
                    .flex()
                    .items_center()
                    .gap_2()
                    .rounded(px(4.))
                    .bg(rgb(fill))
                    .text_size(px(14.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(theme::TEXT_BRIGHT))
                    .child(caption);
                if *disabled {
                    return button.opacity(0.5).into_any_element();
                }
                let button = button.cursor_pointer().hover(|s| s.opacity(0.85));
                match (url, custom_id) {
                    (Some(url), _) => {
                        let url = url.clone();
                        button
                            .on_click(move |_, _, cx| cx.open_url(&url))
                            .child(icon("external-link", 14., theme::TEXT_BRIGHT))
                            .into_any_element()
                    }
                    (None, Some(custom_id)) => {
                        let (message, custom_id) = (message.clone(), custom_id.clone());
                        button
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.click_component(&message, 2, custom_id.clone(), Vec::new(), cx);
                            }))
                            .into_any_element()
                    }
                    (None, None) => button.opacity(0.5).into_any_element(),
                }
            }
            Component::Select { kind, custom_id, placeholder, options, disabled } => {
                self.select(message, key, *kind, custom_id, placeholder.as_deref(), options, *disabled, cx)
            }
            Component::Text(text) => self.render_text(message, text, salt, false, window, cx),
            Component::Section { texts, accessory } => {
                let mut body = div().flex_1().min_w_0().flex().flex_col();
                for text in texts {
                    body = body.child(self.component(message, text, counter, window, cx));
                }
                let side = accessory
                    .as_ref()
                    .map(|a| self.component(message, a, counter, window, cx));
                div().flex().items_start().gap_3().child(body).children(side).into_any_element()
            }
            Component::Thumbnail(media) => {
                let (w, h) = fit(
                    media.width.unwrap_or(0) as f32,
                    media.height.unwrap_or(0) as f32,
                    THUMBNAIL,
                    THUMBNAIL,
                );
                self.media_picture(media, (w, h), key, cx)
            }
            Component::File { media, name } => {
                let url = media.url.clone();
                let label = name
                    .clone()
                    .unwrap_or_else(|| media.url.rsplit('/').next().unwrap_or("file").to_owned());
                div()
                    .id(key)
                    .px_3()
                    .py_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .rounded(px(6.))
                    .bg(rgb(theme::BG_SIDEBAR))
                    .border_1()
                    .border_color(rgb(theme::BORDER))
                    .cursor_pointer()
                    .on_click(move |_, _, cx| cx.open_url(&url))
                    .child(icon("file", 18., theme::TEXT_MUTED))
                    .child(div().text_color(rgb(crate::chat::markdown::LINK_COLOR)).child(label))
                    .into_any_element()
            }
            Component::Gallery(items) => {
                let columns = items.len().clamp(1, 2) as f32;
                let mut row = div().flex().flex_wrap().gap_1();
                for (i, media) in items.iter().enumerate() {
                    let size = fit(
                        media.width.unwrap_or(0) as f32,
                        media.height.unwrap_or(0) as f32,
                        MAX_PICTURE / columns,
                        MAX_PICTURE,
                    );
                    let id = SharedString::from(format!("{key}-img{i}"));
                    row = row.child(self.media_picture(media, size, id, cx));
                }
                row.into_any_element()
            }
            Component::Separator { divider } => div()
                .my_1()
                .h(px(1.))
                .bg(rgb(if *divider { theme::BORDER } else { theme::BG_CHAT }))
                .into_any_element(),
            Component::Container { children, accent } => {
                let mut inner = div().flex_1().min_w_0().p_3().flex().flex_col().gap_2();
                for child in children {
                    inner = inner.child(self.component(message, child, counter, window, cx));
                }
                // The accent is a stripe down the left edge.
                div()
                    .max_w(px(520.))
                    .flex()
                    .overflow_hidden()
                    .rounded(px(8.))
                    .bg(rgb(theme::BG_SIDEBAR))
                    .border_1()
                    .border_color(rgb(theme::BORDER))
                    .child(div().w(px(4.)).flex_none().bg(rgb(accent.unwrap_or(theme::BORDER))))
                    .child(inner)
                    .into_any_element()
            }
            Component::Unknown => div().into_any_element(),
        }
    }

    /// Stickers sent with a message. Lottie ones are vector animations we
    /// cannot play, so they show their name instead.
    pub(super) fn render_stickers(&mut self, message: &Message, cx: &mut Context<Self>) -> Option<AnyElement> {
        if message.sticker_items.is_empty() {
            return None;
        }
        let mut row = div().mt_1().flex().flex_wrap().gap_2();
        for (i, sticker) in message.sticker_items.iter().enumerate() {
            if sticker.is_lottie() {
                row = row.child(
                    div()
                        .px_3()
                        .py_2()
                        .rounded(px(6.))
                        .bg(rgb(theme::BG_SIDEBAR))
                        .text_color(rgb(theme::TEXT_MUTED))
                        .child(format!("Sticker: {}", sticker.name)),
                );
                continue;
            }
            let url = oxicord_media::cdn::sticker(sticker.id, sticker.extension(), 160);
            let id = SharedString::from(format!("sticker-{}-{i}", message.id));
            row = row.child(picture(&self.media, &url, &url, (160., 160.), id, cx));
        }
        Some(row.into_any_element())
    }

    /// Messages forwarded into this one, each in a quoted box.
    pub(super) fn render_forwards(
        &mut self,
        message: &Message,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if message.message_snapshots.is_empty() {
            return None;
        }
        let mut column = div().mt_1().flex().flex_col().gap_1();
        for (i, snapshot) in message.message_snapshots.iter().enumerate() {
            let inner = &snapshot.message;
            // The shared renderers read a whole message, so lend them a copy
            // of this one carrying the forwarded content.
            let mut borrowed = message.clone();
            borrowed.content = inner.content.clone();
            borrowed.attachments = inner.attachments.clone();
            borrowed.embeds = inner.embeds.clone();
            borrowed.mentions = inner.mentions.clone();
            borrowed.sticker_items = inner.sticker_items.clone();
            borrowed.components.clear();
            borrowed.poll = None;
            borrowed.reactions.clear();

            let mut card = div()
                .max_w(px(520.))
                .pl_3()
                .flex()
                .flex_col()
                .gap_1()
                .border_l_2()
                .border_color(rgb(theme::BORDER))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_1()
                        .text_size(px(12.))
                        .text_color(rgb(theme::TEXT_MUTED))
                        .child(icon("forward", 12., theme::TEXT_MUTED))
                        .child("Forwarded"),
                );
            if !borrowed.content.is_empty() {
                card = card.child(self.render_text(&borrowed, &borrowed.content, 1000 + i as u64, false, window, cx));
            }
            if let Some(files) = crate::chat::attachments::render_attachments(&borrowed, &self.media, true, cx) {
                card = card.child(files);
            }
            if let Some(embeds) = crate::chat::attachments::render_embeds(&borrowed, &self.media, true, cx) {
                card = card.child(embeds);
            }
            if let Some(stickers) = self.render_stickers(&borrowed, cx) {
                card = card.child(stickers);
            }
            column = column.child(card);
        }
        Some(column.into_any_element())
    }

    /// A message's poll: the question, each answer with its share of the
    /// votes, and a line saying when it ends.
    pub(super) fn render_poll(&mut self, message: &Message, cx: &mut Context<Self>) -> Option<AnyElement> {
        let poll = message.poll.as_ref()?;
        let total = poll.total_votes();
        let now = chrono::Utc::now();
        let over = poll.finalized || poll.expiry.is_some_and(|e| e <= now);

        let mut card = div()
            .max_w(px(460.))
            .mt_1()
            .p_3()
            .flex()
            .flex_col()
            .gap_2()
            .rounded(px(8.))
            .bg(rgb(theme::BG_SIDEBAR))
            .border_1()
            .border_color(rgb(theme::BORDER))
            .child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(theme::TEXT_BRIGHT))
                    .child(poll.question.clone()),
            )
            .child(
                div().text_size(px(12.)).text_color(rgb(theme::TEXT_MUTED)).child(if poll.multiselect {
                    "Select one or more answers"
                } else {
                    "Select one answer"
                }),
            );

        for answer in &poll.answers {
            let tally = poll.votes(answer.id);
            let share = if total == 0 { 0. } else { tally.count as f32 / total as f32 };
            let (message_id, channel, answer_id) = (message.id, message.channel_id, answer.id);
            let emoji = answer.emoji.as_ref().filter(|e| e.id.is_none()).and_then(|e| e.name.clone());
            let label = match emoji {
                Some(e) => format!("{e} {}", answer.text),
                None => answer.text.clone(),
            };

            let mut row = div()
                .id(SharedString::from(format!("poll-{message_id}-{answer_id}")))
                .relative()
                .h(px(40.))
                .rounded(px(6.))
                .overflow_hidden()
                .border_1()
                .border_color(rgb(if tally.me { theme::ACCENT } else { theme::BORDER }))
                .bg(rgb(theme::BG_PANEL))
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .h_full()
                        .w(relative(share))
                        .bg(rgb(if tally.me { 0x3b4290 } else { theme::BG_ACTIVE })),
                )
                .child(
                    div()
                        .absolute()
                        .size_full()
                        .px_3()
                        .flex()
                        .items_center()
                        .gap_2()
                        .text_color(rgb(theme::TEXT_BRIGHT))
                        .child(div().flex_1().min_w_0().truncate().child(label))
                        .children(tally.me.then(|| icon("check", 14., theme::TEXT_BRIGHT)))
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(rgb(theme::TEXT_MUTED))
                                .child(format!("{} \u{b7} {:.0}%", tally.count, share * 100.)),
                        ),
                );
            if !over {
                row = row.cursor_pointer().hover(|s| s.opacity(0.9)).on_click(cx.listener(
                    move |this, _, _, cx| this.vote(channel, message_id, answer_id, cx),
                ));
            }
            card = card.child(row);
        }

        let footer = match (over, poll.expiry) {
            (true, _) => format!("{total} votes \u{b7} Poll closed"),
            (false, Some(end)) => format!("{total} votes \u{b7} {}", ends_in(end - now)),
            (false, None) => format!("{total} votes"),
        };
        Some(
            card.child(div().text_size(px(12.)).text_color(rgb(theme::TEXT_MUTED)).child(footer))
                .into_any_element(),
        )
    }

    /// Votes for `answer`: shown at once, then sent.
    fn vote(&mut self, channel: Snowflake, message: Snowflake, answer: u32, cx: &mut Context<Self>) {
        let backend = self.backend.clone();
        self.session.update(cx, |session, cx| {
            let Some(poll) = session
                .store
                .timeline(channel)
                .and_then(|t| t.get(message))
                .and_then(|m| m.poll.clone())
            else {
                return;
            };
            let answers = poll.votes_after_picking(answer);
            let changes = session.store.set_my_poll_votes(channel, message, &answers);
            backend.send(Command::VotePoll { channel, message, answers });
            cx.emit(changes);
            cx.notify();
        });
    }

    fn media_picture(
        &mut self,
        media: &MediaRef,
        size: (f32, f32),
        id: SharedString,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let source = media.proxy_url.as_deref().unwrap_or(&media.url);
        picture(&self.media, &resized(source, size.0, size.1), &media.url, size, id, cx)
    }

    /// A drop-down of text options. User, role and channel pickers need a
    /// directory to choose from, so they are shown but left inactive.
    #[allow(clippy::too_many_arguments)]
    fn select(
        &mut self,
        message: &Message,
        key: SharedString,
        kind: u8,
        custom_id: &str,
        placeholder: Option<&str>,
        options: &[oxicord_model::SelectOption],
        disabled: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let slot = (message.id, custom_id.to_owned());
        let open = self.open_menu.as_ref() == Some(&slot);
        let shown = self
            .picked
            .get(&slot)
            .and_then(|value| options.iter().find(|o| &o.value == value))
            .or_else(|| options.iter().find(|o| o.default))
            .map(|o| o.label.clone())
            .or_else(|| placeholder.map(str::to_owned))
            .unwrap_or_else(|| "Select".to_owned());

        let head = div()
            .id(key.clone())
            .w(px(320.))
            .max_w_full()
            .h(px(36.))
            .px_3()
            .flex()
            .items_center()
            .justify_between()
            .rounded(px(4.))
            .border_1()
            .border_color(rgb(theme::BORDER))
            .bg(rgb(theme::BG_SIDEBAR))
            .text_color(rgb(theme::TEXT))
            .child(div().truncate().child(shown))
            .child(icon(if open { "chevron-up" } else { "chevron-down" }, 16., theme::TEXT_MUTED));
        if kind != 3 || disabled {
            return head.opacity(0.5).into_any_element();
        }

        let toggle = slot.clone();
        let head = head.cursor_pointer().on_click(cx.listener(move |this, _, _, cx| {
            this.open_menu = (this.open_menu.as_ref() != Some(&toggle)).then(|| toggle.clone());
            cx.notify();
        }));

        let mut menu = div().flex().flex_col().gap(px(2.)).child(head);
        if open {
            let mut list = div()
                .w(px(320.))
                .max_w_full()
                .p_1()
                .flex()
                .flex_col()
                .rounded(px(4.))
                .border_1()
                .border_color(rgb(theme::BORDER))
                .bg(rgb(theme::BG_PANEL));
            for (i, option) in options.iter().enumerate() {
                let (message, slot, value) = (message.clone(), slot.clone(), option.value.clone());
                list = list.child(
                    div()
                        .id(SharedString::from(format!("{key}-opt{i}")))
                        .px_2()
                        .py(px(6.))
                        .flex()
                        .flex_col()
                        .rounded(px(4.))
                        .cursor_pointer()
                        .hover(|s| s.bg(rgb(theme::BG_HOVER)))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.open_menu = None;
                            this.picked.insert(slot.clone(), value.clone());
                            this.click_component(&message, 3, slot.1.clone(), vec![value.clone()], cx);
                        }))
                        .child(div().text_color(rgb(theme::TEXT_BRIGHT)).child(option.label.clone()))
                        .children(option.description.clone().map(|d| {
                            div().text_size(px(12.)).text_color(rgb(theme::TEXT_MUTED)).child(d)
                        })),
                );
            }
            menu = menu.child(list);
        }
        menu.into_any_element()
    }

    /// Sends a click on one of `message`'s components to its bot.
    fn click_component(
        &mut self,
        message: &Message,
        component_type: u8,
        custom_id: String,
        values: Vec<String>,
        cx: &mut Context<Self>,
    ) {
        let Some(session_id) = self.session.read(cx).store.session_id().map(str::to_owned) else {
            return;
        };
        let application: Snowflake = owner(message.application_id, message.author.id);
        self.backend.send(Command::ClickComponent {
            guild: self.guild(cx),
            channel: message.channel_id,
            message: message.id,
            message_flags: message.flags,
            application,
            session_id,
            component_type,
            custom_id,
            values,
        });
    }
}

/// "Ends in 3 hours", with the largest sensible unit.
fn ends_in(left: chrono::Duration) -> String {
    let unit = |n: i64, name: &str| format!("Ends in {n} {name}{}", if n == 1 { "" } else { "s" });
    match left.num_seconds() {
        s if s < 60 => "Ends in under a minute".to_owned(),
        s if s < 3600 => unit(s / 60, "minute"),
        s if s < 86_400 => unit(s / 3600, "hour"),
        s => unit(s / 86_400, "day"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_time_left_uses_the_largest_unit() {
        assert_eq!(ends_in(chrono::Duration::seconds(20)), "Ends in under a minute");
        assert_eq!(ends_in(chrono::Duration::minutes(1)), "Ends in 1 minute");
        assert_eq!(ends_in(chrono::Duration::hours(5)), "Ends in 5 hours");
        assert_eq!(ends_in(chrono::Duration::days(2)), "Ends in 2 days");
    }
}
