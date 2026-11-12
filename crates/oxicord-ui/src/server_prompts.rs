//! The two small modals behind the server menu: the Leave Server confirm and
//! the Server Tag one that offers to wear the server's tag. Both are drawn
//! from a live capture at 1920 x 1080. Leaving and adopting write to the
//! account, so they are built and unit tested here, never sent while checking.

use gpui_kit::{
    AnyElement, ClickEvent, Context, Entity, EventEmitter, FontWeight, HighlightStyle, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled as _, StyledText,
    Window, div, img, px, rgb, rgba,
};
use gpui_kit::prelude::FluentBuilder as _;
use oxicord_media::cdn;
use oxicord_model::{ServerTag, Snowflake};

use crate::{
    backend::{Backend, Command},
    icons::icon,
    media::Media,
    profile::modal,
    session::{OpenServerProfile, Session},
    theme,
};

/// Asked of the session by the header menu's Leave Server row.
pub struct OpenLeaveServer(pub Snowflake);

impl EventEmitter<OpenLeaveServer> for Session {}

/// Asked of the session by the header menu's Server Tag row.
pub struct OpenServerTag(pub Snowflake);

impl EventEmitter<OpenServerTag> for Session {}

pub struct CloseServerPrompt;

impl EventEmitter<CloseServerPrompt> for ServerPrompt {}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Leave,
    Tag,
}

pub struct ServerPrompt {
    session: Entity<Session>,
    media: Entity<Media>,
    backend: Backend,
    shown: Option<(Kind, Snowflake)>,
}

/// The pale body text of the tag modal and its mock chat.
const BODY: u32 = 0xd4d5d8;
/// The three sample people beside the tag, with their live name colors.
const SAMPLE: [(&str, &str, u32, bool); 5] = [
    ("Roka", "anyone down for aram", 0x7487f8, false),
    ("hongo", "count me in", 0x92dba6, false),
    ("Lily", "check out my tag!", 0xf1a579, true),
    ("sharon", "woah how did you get that", 0x7487f8, false),
    ("Lily", "anyone here can get it!", 0xf1a579, false),
];

impl ServerPrompt {
    pub fn new(session: Entity<Session>, media: Entity<Media>, backend: Backend) -> Self {
        Self { session, media, backend, shown: None }
    }

    pub fn leave(&mut self, guild: Snowflake, cx: &mut Context<Self>) {
        self.shown = Some((Kind::Leave, guild));
        cx.notify();
    }

    pub fn tag(&mut self, guild: Snowflake, cx: &mut Context<Self>) {
        self.shown = Some((Kind::Tag, guild));
        cx.notify();
    }

    fn close(&mut self, cx: &mut Context<Self>) {
        self.shown = None;
        cx.emit(CloseServerPrompt);
    }

    fn button(id: &'static str, label: &'static str, fill: Fill) -> gpui_kit::Stateful<gpui_kit::Div> {
        let (bg, border, text) = match fill {
            Fill::Accent => (rgb(theme::ACCENT), rgba(0xffffff14), 0xffffff),
            Fill::Danger => (rgb(0xd22d39), rgba(0xffffff14), 0xffffff),
            Fill::Plain => (rgba(0x97979f1f), rgba(0x97979f29), 0xdcdcdf),
        };
        div()
            .id(id)
            .h(px(40.))
            .flex_1()
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(8.))
            .border_1()
            .border_color(border)
            .bg(bg)
            .cursor_pointer()
            .hover(|s| s.opacity(0.85))
            .text_size(px(16.))
            .line_height(px(20.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(rgb(text))
            .child(label)
    }

    fn close_button(&self, id: &'static str, size: f32, top: f32, right: f32, cx: &mut Context<Self>) -> AnyElement {
        div()
            .id(id)
            .absolute()
            .top(px(top))
            .right(px(right))
            .size(px(size))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(8.))
            .cursor_pointer()
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close(cx)))
            .child(icon("x", if size > 32. { 20. } else { 24. }, if size > 32. { theme::TEXT_MUTED } else { theme::TEXT_FAINT }))
            .into_any_element()
    }

    fn leave_card(&self, guild: Snowflake, cx: &mut Context<Self>) -> AnyElement {
        let name = self.session.read(cx).store.guild(guild).map(|g| g.name.clone()).unwrap_or_default();
        let (text, bold) = leave_text(&name);
        let styled = StyledText::new(text).with_highlights([(
            bold,
            HighlightStyle { font_weight: Some(FontWeight::SEMIBOLD), ..Default::default() },
        )]);

        div()
            .id("leave-card")
            .occlude()
            .relative()
            .w(px(400.))
            .pt(px(8.))
            .pb(px(16.))
            .flex()
            .flex_col()
            .rounded(px(12.))
            .border_1()
            .border_color(rgb(theme::BORDER_OVERLAY))
            .bg(rgb(theme::BG_PANEL))
            .child(
                div()
                    .px(px(24.))
                    .pt(px(16.))
                    .pb(px(8.))
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(
                        div()
                            .pr(px(36.))
                            .text_size(px(20.))
                            .line_height(px(24.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(theme::TEXT_BRIGHT))
                            .child(format!("Leave '{name}'")),
                    )
                    .child(
                        div()
                            .text_size(px(16.))
                            .line_height(px(20.))
                            .text_color(rgb(theme::TEXT_MUTED))
                            .child(styled),
                    ),
            )
            .child(
                div()
                    .h(px(64.))
                    .px(px(24.))
                    .pt(px(16.))
                    .pb(px(8.))
                    .flex()
                    .gap(px(8.))
                    .child(
                        Self::button("leave-cancel", "Cancel", Fill::Plain)
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close(cx))),
                    )
                    .child(Self::button("leave-confirm", "Leave Server", Fill::Danger).on_click(cx.listener(
                        move |this, _: &ClickEvent, _, cx| {
                            this.backend.send(Command::LeaveGuild(guild));
                            this.close(cx);
                        },
                    ))),
            )
            .child(self.close_button("leave-close", 40., 8., 8., cx))
            .into_any_element()
    }

    /// The tag chip inside the server row: a 26px box with the badge and the letters.
    fn big_chip(&self, tag: &ServerTag, cx: &mut Context<Self>) -> AnyElement {
        let picture = tag
            .badge
            .as_deref()
            .and_then(|hash| self.media.update(cx, |m, _| m.get(&cdn::clan_badge(tag.guild_id, hash, 32))));
        div()
            .flex_none()
            .h(px(26.))
            .px(px(8.))
            .flex()
            .items_center()
            .gap(px(2.))
            .rounded(px(4.))
            .border_1()
            .border_color(rgba(0x97979f70))
            .bg(rgb(theme::BG_PANEL))
            .text_size(px(14.))
            .line_height(px(16.))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(rgb(theme::TEXT_MUTED))
            .children(picture.map(|image| img(gpui_kit::ImageSource::Image(image)).size(px(16.)).flex_none()))
            .child(tag.tag.clone())
            .into_any_element()
    }

    fn tag_card(&self, guild: Snowflake, cx: &mut Context<Self>) -> AnyElement {
        let (name, icon_hash, tag, adopted) = {
            let store = &self.session.read(cx).store;
            let guild_row = store.guild(guild);
            (
                guild_row.map(|g| g.name.clone()).unwrap_or_default(),
                guild_row.and_then(|g| g.icon.clone()),
                self.session.read(cx).guild_tags.get(&guild).cloned().flatten(),
                store.me().and_then(|me| me.server_tag()).is_some_and(|t| t.guild_id == guild),
            )
        };
        let picture = icon_hash
            .and_then(|hash| self.media.update(cx, |m, _| m.get(&cdn::guild_icon(guild, &hash, 64))));
        let initial = name.chars().next().map(|c| c.to_string()).unwrap_or_default();

        let server = div()
            .h(px(50.))
            .p(px(8.))
            .flex()
            .items_center()
            .gap(px(8.))
            .rounded(px(8.))
            .border_1()
            .border_color(rgba(0x97979f33))
            .bg(rgb(0x000000))
            .child(match picture {
                Some(image) => img(gpui_kit::ImageSource::Image(image)).size(px(32.)).rounded_full().flex_none().into_any_element(),
                None => div()
                    .size(px(32.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_full()
                    .bg(rgb(theme::BG_ACTIVE))
                    .text_size(px(14.))
                    .text_color(rgb(theme::TEXT_BRIGHT))
                    .child(initial)
                    .into_any_element(),
            })
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_size(px(16.))
                    .line_height(px(20.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(theme::TEXT_BRIGHT))
                    .child(name),
            )
            .children(tag.as_ref().map(|tag| self.big_chip(tag, cx)));

        let adopt = Self::button("tag-adopt", "Adopt Tag", Fill::Accent)
            .flex_none()
            .w_full()
            .when(adopted, |s| s.opacity(0.5).cursor_default())
            .when(!adopted, |s| {
                s.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.backend.send(Command::AdoptServerTag { guild, enabled: true });
                    this.close(cx);
                }))
            });
        let edit = Self::button("tag-edit", "Edit Profile", Fill::Plain)
            .flex_none()
            .w_full()
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.session.update(cx, |_, cx| cx.emit(OpenServerProfile(guild)));
                this.close(cx);
            }));

        let left = div()
            .w(px(364.))
            .flex_none()
            .px(px(24.))
            .py(px(35.))
            .flex()
            .flex_col()
            .gap(px(24.))
            .child(
                div()
                    .h(px(64.))
                    .child(
                        div()
                            .text_size(px(20.))
                            .line_height(px(24.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(theme::TEXT_BRIGHT))
                            .child("Adopt this server's tag"),
                    )
                    .child(
                        div()
                            .text_size(px(16.))
                            .line_height(px(20.))
                            .text_color(rgb(BODY))
                            .child("Represent your favorite server, everywhere you go on Discord."),
                    ),
            )
            .child(server)
            .child(div().flex().flex_col().gap(px(8.)).child(adopt).child(edit));

        let chip = tag.clone();
        let chat = div()
            .w(px(312.))
            .p(px(12.))
            .flex()
            .flex_col()
            .gap(px(12.))
            .rounded(px(4.))
            .children(SAMPLE.iter().map(|&(who, said, color, wears)| {
                let line = div()
                    .h(px(20.))
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .text_size(px(16.))
                    .line_height(px(20.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(color))
                    .child(who)
                    .children(chip.clone().filter(|_| wears).map(|t| crate::member_list::server_tag_chip(&t, 14., &self.media, cx)));
                div()
                    .h(px(40.))
                    .flex()
                    .gap(px(12.))
                    .child(
                        img(SharedString::from(format!("tag-preview/{}.png", who.to_lowercase())))
                            .size(px(40.))
                            .rounded_full()
                            .flex_none(),
                    )
                    .child(
                        div().flex().flex_col().child(line).child(
                            div()
                                .h(px(20.))
                                .text_size(px(16.))
                                .line_height(px(20.))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(rgb(BODY))
                                .child(said),
                        ),
                    )
            }));
        let right = div().w(px(340.)).flex_none().pt(px(24.)).pl(px(26.)).bg(rgb(0x000000)).child(chat);

        div()
            .id("tag-card")
            .occlude()
            .relative()
            .flex()
            .h(px(322.))
            .w(px(706.))
            .overflow_hidden()
            .rounded(px(12.))
            .border_1()
            .border_color(rgb(theme::BORDER_OVERLAY))
            .bg(rgb(theme::BG_PANEL))
            .child(left)
            .child(right)
            .child(self.close_button("tag-close", 32., 16., 16., cx))
            .into_any_element()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Fill {
    Accent,
    Danger,
    Plain,
}

/// The sentence under the Leave title and the span of the server name in it.
fn leave_text(name: &str) -> (String, std::ops::Range<usize>) {
    let head = "Are you sure you want to leave ";
    (
        format!("{head}{name}? You won't be able to rejoin this server unless you are re-invited."),
        head.len()..head.len() + name.len(),
    )
}

impl Render for ServerPrompt {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some((kind, guild)) = self.shown else { return div().into_any_element() };
        let card = match kind {
            Kind::Leave => self.leave_card(guild, cx),
            Kind::Tag => self.tag_card(guild, cx),
        };
        modal(card, Box::new(cx.listener(|this, _: &ClickEvent, _, cx| this.close(cx))))
    }
}

#[cfg(test)]
mod tests {
    use super::leave_text;

    #[test]
    fn the_leave_sentence_marks_the_server_name() {
        let (text, bold) = leave_text("Low iq gaymers");
        assert_eq!(
            text,
            "Are you sure you want to leave Low iq gaymers? You won't be able to rejoin this server unless you are re-invited."
        );
        assert_eq!(&text[bold], "Low iq gaymers");
    }

    #[test]
    fn a_name_with_multibyte_letters_keeps_a_valid_span() {
        let (text, bold) = leave_text("Café ☕");
        assert_eq!(&text[bold], "Café ☕");
    }
}
