//! The Events modal a server's "N Events" row opens: one card per event that
//! is running or still to come, laid out as the web client draws it.

use chrono::{DateTime, Datelike as _, Local, TimeZone};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, ClickEvent, ClipboardItem, Context, ElementId, Entity, EventEmitter, FontWeight,
    InteractiveElement as _, IntoElement, ObjectFit, ParentElement as _, Render, ScrollHandle, SharedString,
    StatefulInteractiveElement as _, Styled as _, StyledImage as _, Window, div, img, px, rgb,
};
use oxicord_media::cdn;
use oxicord_model::{EventPlace, EventStatus, ScheduledEvent, Snowflake};

use crate::{
    chat::markdown::rich,
    icons::icon,
    media::Media,
    profile::modal,
    scroll,
    session::Session,
    theme,
};

/// The calendar glyph of an event that has not started.
const UPCOMING: u32 = 0x6374f4;
/// A card's border: rgba(150,151,159,0.2) over `BG_CARD`.
const CARD_BORDER: u32 = 0x303135;
const FRAME_WIDTH: f32 = 604.;
const FRAME_HEIGHT: f32 = 802.;
const COVER_HEIGHT: f32 = 120.;
/// The description stops after four 18 high lines.
const DESCRIPTION_HEIGHT: f32 = 72.;

pub struct CloseEvents;

impl EventEmitter<CloseEvents> for EventsDialog {}

pub struct EventsDialog {
    session: Entity<Session>,
    media: Entity<Media>,
    guild: Option<Snowflake>,
    scroll: ScrollHandle,
}

impl EventsDialog {
    pub fn new(session: Entity<Session>, media: Entity<Media>) -> Self {
        Self { session, media, guild: None, scroll: ScrollHandle::new() }
    }

    pub fn opened(&mut self, guild: Snowflake, cx: &mut Context<Self>) {
        self.guild = Some(guild);
        self.scroll.set_offset(gpui_kit::point(px(0.), px(0.)));
        cx.notify();
    }

    fn close(&mut self, cx: &mut Context<Self>) {
        cx.emit(CloseEvents);
    }

    fn card(&mut self, event: &ScheduledEvent, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let active = event.status() == EventStatus::Active;
        let (status_color, status) = if active {
            (theme::STATUS_POSITIVE, "Happening Now".to_owned())
        } else {
            (UPCOMING, start_label(&event.scheduled_start_time.with_timezone(&Local)))
        };

        let cover = event.image.as_deref().map(|hash| {
            let url = cdn::event_cover(event.id, hash, 1024);
            let image = self.media.update(cx, |media, _| media.get(&url));
            div()
                .h(px(COVER_HEIGHT))
                .w_full()
                .flex_none()
                .overflow_hidden()
                .rounded_t(px(7.))
                .bg(rgb(theme::BG_PANEL))
                .when_some(image, |cover, image| {
                    cover.child(
                        img(gpui_kit::ImageSource::Image(image))
                            .w_full()
                            .h(px(COVER_HEIGHT))
                            .rounded_t(px(7.))
                            .object_fit(ObjectFit::Cover),
                    )
                })
        });

        let description = event
            .description
            .as_deref()
            .filter(|text| !text.trim().is_empty())
            .map(|text| {
                let key = event.id.0 ^ 0xE7E7;
                let names = Default::default();
                div()
                    .mt(px(8.))
                    .max_h(px(DESCRIPTION_HEIGHT))
                    .overflow_hidden()
                    .text_size(px(14.))
                    .line_height(px(18.))
                    .text_color(rgb(theme::TEXT_BRIGHT))
                    .child(rich(text, key, &self.media, &names, window, cx))
            });

        let place: Option<(&str, SharedString)> = match event.place() {
            EventPlace::External => event.location().map(|text| ("location-pin", text.to_owned().into())),
            EventPlace::Voice | EventPlace::Stage => event
                .channel_id
                .and_then(|channel| {
                    self.session.read(cx).store.channel(channel).and_then(|c| c.name.clone())
                })
                .map(|name| ("volume-2", name.into())),
        };

        let (guild, id) = (event.guild_id, event.id);
        let copy = button(
            ElementId::Name(format!("event-copy-{id}").into()),
            Some("Copy Link"),
            "link",
            cx.listener(move |_, _: &ClickEvent, _, cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(event_link(guild, id)));
            }),
        );

        let footer = div()
            .px(px(16.))
            .pt(px(12.))
            .pb(px(16.))
            .flex()
            .items_center()
            .justify_between()
            .gap(px(8.))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .children(place.map(|(glyph, text)| {
                        div()
                            .flex()
                            .items_center()
                            .gap(px(4.))
                            .min_w_0()
                            .child(icon(glyph, 20., theme::TEXT_MUTED))
                            .child(
                                div()
                                    .min_w_0()
                                    .truncate()
                                    .text_size(px(14.))
                                    .line_height(px(18.))
                                    .text_color(rgb(theme::TEXT_BRIGHT))
                                    .child(text),
                            )
                    })),
            )
            .child(
                div()
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(button(
                        ElementId::Name(format!("event-more-{id}").into()),
                        None,
                        "ellipsis",
                        |_: &ClickEvent, _, _| {},
                    ))
                    .child(copy)
                    .when(!active, |buttons| {
                        buttons.child(button(
                            ElementId::Name(format!("event-interested-{id}").into()),
                            Some("Interested"),
                            "bell",
                            |_: &ClickEvent, _, _| {},
                        ))
                    }),
            );

        div()
            .w_full()
            .flex_none()
            .flex()
            .flex_col()
            .rounded(px(8.))
            .border_1()
            .border_color(rgb(CARD_BORDER))
            .bg(rgb(theme::BG_CARD))
            .children(cover)
            .child(
                div()
                    .p(px(16.))
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .h(px(20.))
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(icon("calendar", 20., status_color))
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .line_height(px(18.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(if active { theme::STATUS_POSITIVE } else { theme::TEXT_BRIGHT }))
                                    .child(status),
                            ),
                    )
                    .child(
                        div()
                            .mt(px(12.))
                            .text_size(px(20.))
                            .line_height(px(24.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgb(theme::TEXT_BRIGHT))
                            .child(event.name.clone()),
                    )
                    .children(description),
            )
            .child(div().h(px(1.)).w_full().bg(rgb(CARD_BORDER)))
            .child(footer)
            .into_any_element()
    }
}

/// Where the web client sends people to see an event.
fn event_link(guild: Snowflake, event: Snowflake) -> String {
    format!("https://discord.com/events/{guild}/{event}")
}

fn ordinal(day: u32) -> &'static str {
    match (day % 100, day % 10) {
        (11..=13, _) => "th",
        (_, 1) => "st",
        (_, 2) => "nd",
        (_, 3) => "rd",
        _ => "th",
    }
}

/// "Fri Nov 20th · 7:00 PM", the way the web client words a start time.
fn start_label<Tz: TimeZone>(start: &DateTime<Tz>) -> String
where
    Tz::Offset: std::fmt::Display,
{
    let day = start.day();
    format!(
        "{} {day}{} \u{b7} {}",
        start.format("%a %b"),
        ordinal(day),
        start.format("%-I:%M %p")
    )
}

/// A 32 high secondary button: just a glyph, or a glyph and its label.
fn button(
    id: ElementId,
    label: Option<&'static str>,
    glyph: &'static str,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut gpui_kit::App) + 'static,
) -> AnyElement {
    div()
        .id(id)
        .h(px(32.))
        .min_w(px(32.))
        .px(px(if label.is_some() { 12. } else { 0. }))
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .gap(px(4.))
        .rounded(px(8.))
        .border_1()
        .border_color(rgb(theme::BORDER_OVERLAY))
        .bg(rgb(theme::SECONDARY))
        .cursor_pointer()
        .hover(|s| s.bg(rgb(theme::SECONDARY_HOVER)))
        .on_click(on_click)
        .child(icon(glyph, 16., theme::TEXT_BRIGHT))
        .children(label.map(|text| {
            div()
                .text_size(px(14.))
                .line_height(px(18.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(rgb(theme::TEXT_BRIGHT))
                .child(text)
        }))
        .into_any_element()
}

impl Render for EventsDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut events: Vec<ScheduledEvent> = self
            .guild
            .and_then(|guild| self.session.read(cx).store.guild(guild))
            .map(|guild| guild.scheduled_events.clone())
            .unwrap_or_default();
        events.retain(|event| matches!(event.status(), EventStatus::Active | EventStatus::Scheduled));
        // What is on now comes first, then by start.
        events.sort_by_key(|event| (event.status() != EventStatus::Active, event.scheduled_start_time));

        let title = format!("{} Event{}", events.len(), if events.len() == 1 { "" } else { "s" });
        let mut list = div().px(px(24.)).pt(px(8.)).pb(px(24.)).flex().flex_col().gap(px(16.));
        for event in &events {
            list = list.child(self.card(event, window, cx));
        }

        let frame = div()
            .id("events-dialog")
            .occlude()
            .w(px(FRAME_WIDTH))
            .max_h(px(FRAME_HEIGHT))
            .flex()
            .flex_col()
            .overflow_hidden()
            .rounded(px(12.))
            .border_1()
            .border_color(rgb(theme::BORDER_OVERLAY))
            .bg(rgb(theme::BG_PANEL))
            .child(
                div()
                    .h(px(48.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_between()
                    .pt(px(8.))
                    .px(px(24.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(icon("calendar", 24., theme::TEXT_MUTED))
                            .child(
                                div()
                                    .text_size(px(16.))
                                    .line_height(px(20.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(theme::TEXT_BRIGHT))
                                    .child(title),
                            ),
                    )
                    .child(
                        div()
                            .id("close-events")
                            .size(px(24.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close(cx)))
                            .child(icon("x", 20., theme::TEXT_MUTED)),
                    ),
            )
            .child(scroll::vertical("events-body", &self.scroll, list));

        modal(
            frame.into_any_element(),
            Box::new(cx.listener(|this, _: &ClickEvent, _, cx| this.close(cx))),
        )
    }
}

#[cfg(test)]
mod tests {
    use chrono::FixedOffset;

    use super::*;

    #[test]
    fn a_start_time_is_worded_like_the_web_client() {
        let start = FixedOffset::east_opt(0)
            .unwrap()
            .with_ymd_and_hms(2026, 11, 20, 19, 0, 0)
            .unwrap();
        assert_eq!(start_label(&start), "Fri Nov 20th \u{b7} 7:00 PM");
        let morning = FixedOffset::east_opt(0)
            .unwrap()
            .with_ymd_and_hms(2026, 12, 1, 9, 5, 0)
            .unwrap();
        assert_eq!(start_label(&morning), "Tue Dec 1st \u{b7} 9:05 AM");
    }

    #[test]
    fn days_take_their_ordinal() {
        let words: Vec<_> = [1, 2, 3, 4, 11, 12, 13, 21, 22, 23, 24, 30]
            .iter()
            .map(|day| ordinal(*day))
            .collect();
        assert_eq!(words, ["st", "nd", "rd", "th", "th", "th", "th", "st", "nd", "rd", "th", "th"]);
    }

    #[test]
    fn the_link_names_the_server_and_event() {
        assert_eq!(event_link(Snowflake(1), Snowflake(2)), "https://discord.com/events/1/2");
    }
}
