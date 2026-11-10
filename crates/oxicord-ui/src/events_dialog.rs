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
    backend::{Backend, Command},
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
const DETAIL_WIDTH: f32 = 680.;
const DETAIL_HEIGHT: f32 = 952.;
const DETAIL_COVER_HEIGHT: f32 = 240.;
const COVER_HEIGHT: f32 = 120.;
/// The description stops after four 18 high lines.
const DESCRIPTION_HEIGHT: f32 = 72.;

pub struct CloseEvents;

impl EventEmitter<CloseEvents> for EventsDialog {}

/// The event whose details are open, and whether the list sits behind it.
#[derive(Clone, Copy)]
struct Detail {
    event: Snowflake,
    from_list: bool,
}

pub struct EventsDialog {
    session: Entity<Session>,
    media: Entity<Media>,
    backend: Backend,
    guild: Option<Snowflake>,
    detail: Option<Detail>,
    scroll: ScrollHandle,
    detail_scroll: ScrollHandle,
}

impl EventsDialog {
    pub fn new(session: Entity<Session>, media: Entity<Media>, backend: Backend) -> Self {
        Self {
            session,
            media,
            backend,
            guild: None,
            detail: None,
            scroll: ScrollHandle::new(),
            detail_scroll: ScrollHandle::new(),
        }
    }

    pub fn opened(&mut self, guild: Snowflake, cx: &mut Context<Self>) {
        self.guild = Some(guild);
        self.detail = None;
        self.scroll.set_offset(gpui_kit::point(px(0.), px(0.)));
        self.ask_counts(cx);
        cx.notify();
    }

    /// Opens one event's details straight away, with no list behind it.
    pub fn opened_event(&mut self, guild: Snowflake, event: Snowflake, cx: &mut Context<Self>) {
        self.guild = Some(guild);
        self.ask_counts(cx);
        self.show(event, false, cx);
    }

    /// Asks how many people are interested in each of the server's events.
    fn ask_counts(&self, cx: &mut Context<Self>) {
        let Some(guild) = self.guild else { return };
        let ids: Vec<Snowflake> = self
            .session
            .read(cx)
            .store
            .guild(guild)
            .map(|guild| guild.scheduled_events.iter().map(|event| event.id).collect())
            .unwrap_or_default();
        for event in ids {
            self.backend.send(Command::LoadEventCount { guild, event });
        }
    }

    fn show(&mut self, event: Snowflake, from_list: bool, cx: &mut Context<Self>) {
        self.detail = Some(Detail { event, from_list });
        self.detail_scroll.set_offset(gpui_kit::point(px(0.), px(0.)));
        // The creator is often someone we have never seen.
        let creator = self.guild.and_then(|guild| {
            let session = self.session.read(cx);
            let creator = session
                .store
                .guild(guild)?
                .scheduled_events
                .iter()
                .find(|e| e.id == event)?
                .creator_id?;
            session.store.user(creator).is_none().then_some((guild, creator))
        });
        if let Some((guild, creator)) = creator {
            self.backend.send(Command::RequestMembers { guild, ids: vec![creator] });
        }
        cx.notify();
    }

    /// Closes the details and shows the list again, or closes the whole modal.
    fn close(&mut self, cx: &mut Context<Self>) {
        match self.detail {
            Some(Detail { from_list: true, .. }) => {
                self.detail = None;
                cx.notify();
            }
            _ => cx.emit(CloseEvents),
        }
    }

    fn card(&mut self, event: &ScheduledEvent, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let active = event.status() == EventStatus::Active;
        let interested = self.session.read(cx).event_counts.get(&event.id).copied();
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
            .id(ElementId::Name(format!("event-card-{id}").into()))
            .w_full()
            .flex_none()
            .flex()
            .flex_col()
            .rounded(px(8.))
            .border_1()
            .border_color(rgb(CARD_BORDER))
            .bg(rgb(theme::BG_CARD))
            .cursor_pointer()
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.show(id, true, cx)))
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
                            .justify_between()
                            .gap(px(8.))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.))
                                    .child(icon("calendar", 20., status_color))
                                    .child(
                                        div()
                                            .text_size(px(14.))
                                            .line_height(px(18.))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(rgb(if active {
                                                theme::STATUS_POSITIVE
                                            } else {
                                                theme::TEXT_BRIGHT
                                            }))
                                            .child(status),
                                    ),
                            )
                            .children(interested.map(|count| {
                                div()
                                    .h(px(20.))
                                    .px(px(8.))
                                    .flex()
                                    .flex_none()
                                    .items_center()
                                    .gap(px(4.))
                                    .rounded(px(12.))
                                    .bg(rgb(0x000000))
                                    .child(icon("people", 12., theme::TEXT_BRIGHT))
                                    .child(
                                        div()
                                            .text_size(px(14.))
                                            .line_height(px(18.))
                                            .text_color(rgb(theme::TEXT_BRIGHT))
                                            .child(count.to_string()),
                                    )
                            })),
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

impl EventsDialog {
    /// One line of the info list: a 20 high glyph or picture, then text.
    fn info_row(glyph: AnyElement, text: AnyElement) -> AnyElement {
        div()
            .h(px(20.))
            .flex()
            .items_center()
            .gap(px(8.))
            .child(glyph)
            .child(text)
            .into_any_element()
    }

    fn info_text(text: impl Into<SharedString>) -> AnyElement {
        div()
            .text_size(px(14.))
            .line_height(px(18.))
            .text_color(rgb(theme::TEXT_BRIGHT))
            .child(text.into())
            .into_any_element()
    }

    /// The details of one event, over the list when it was opened from there.
    fn details(&mut self, event: &ScheduledEvent, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let active = event.status() == EventStatus::Active;
        let when = if active {
            "Happening Now".to_owned()
        } else {
            let start = start_label(&event.scheduled_start_time.with_timezone(&Local));
            match event.scheduled_end_time {
                Some(end) => format!("{start} \u{2014} {}", start_label(&end.with_timezone(&Local))),
                None => start,
            }
        };

        let cover = event.image.as_deref().map(|hash| {
            let url = cdn::event_cover(event.id, hash, 1024);
            let image = self.media.update(cx, |media, _| media.get(&url));
            div()
                .h(px(DETAIL_COVER_HEIGHT))
                .w_full()
                .flex_none()
                .overflow_hidden()
                .rounded(px(8.))
                .bg(rgb(theme::BG_CARD))
                .when_some(image, |cover, image| {
                    cover.child(
                        img(gpui_kit::ImageSource::Image(image))
                            .w_full()
                            .h(px(DETAIL_COVER_HEIGHT))
                            .rounded(px(8.))
                            .object_fit(ObjectFit::Cover),
                    )
                })
        });

        let interested = self.session.read(cx).event_counts.get(&event.id).copied();
        let tab = |label: String, selected: bool| {
            div()
                .flex_none()
                .flex()
                .flex_col()
                .child(
                    div()
                        .h(px(20.))
                        .text_size(px(16.))
                        .line_height(px(20.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(rgb(if selected { theme::TEXT_BRIGHT } else { theme::TEXT_MUTED }))
                        .child(label),
                )
                .child(
                    div()
                        .mt(px(14.))
                        .h(px(2.))
                        .w_full()
                        .rounded_full()
                        .when(selected, |bar| bar.bg(rgb(theme::ACCENT))),
                )
        };

        let mut rows = Vec::new();
        let guild = self.session.read(cx).store.guild(event.guild_id).map(|g| (g.name.clone(), g.icon.clone()));
        if let Some((name, icon_hash)) = guild {
            let image = icon_hash.and_then(|hash| {
                let url = cdn::guild_icon(event.guild_id, &hash, 40);
                self.media.update(cx, |media, _| media.get(&url))
            });
            let badge = div()
                .size(px(20.))
                .flex_none()
                .overflow_hidden()
                .rounded(px(6.))
                .bg(rgb(theme::ACCENT))
                .when_some(image, |badge, image| {
                    badge.child(img(gpui_kit::ImageSource::Image(image)).size_full().rounded(px(6.)))
                });
            rows.push(Self::info_row(badge.into_any_element(), Self::info_text(name)));
        }
        let place = match event.place() {
            EventPlace::External => event.location().map(|text| ("location-pin", text.to_owned())),
            EventPlace::Voice | EventPlace::Stage => event
                .channel_id
                .and_then(|channel| self.session.read(cx).store.channel(channel).and_then(|c| c.name.clone()))
                .map(|name| ("volume-2", name)),
        };
        if let Some((glyph, text)) = place {
            rows.push(Self::info_row(icon(glyph, 20., theme::TEXT_MUTED).into_any_element(), Self::info_text(text)));
        }
        if let Some(count) = interested {
            let text = match count {
                1 => "1 person is interested".to_owned(),
                n => format!("{} people are interested", thousands(n)),
            };
            rows.push(Self::info_row(icon("people", 20., theme::TEXT_MUTED).into_any_element(), Self::info_text(text)));
        }
        let creator = event
            .creator_id
            .and_then(|id| self.session.read(cx).store.user(id).cloned());
        if let Some(user) = creator {
            let url = cdn::user_avatar(user.id, user.avatar.as_deref(), 40);
            let avatar = crate::media::avatar(&self.media, &url, 20., user.display_name(), cx).into_any_element();
            rows.push(Self::info_row(avatar, Self::info_text(format!("Created by {}", user.display_name()))));
        }

        let key = event.id.0 ^ 0xDE7A;
        let description = event
            .description
            .as_deref()
            .filter(|text| !text.trim().is_empty())
            .map(|text| {
                let names = Default::default();
                div()
                    .mt(px(16.))
                    .text_size(px(14.))
                    .line_height(px(18.))
                    .text_color(rgb(theme::TEXT_BRIGHT))
                    .child(rich(text, key, &self.media, &names, window, cx))
            });

        let (guild_id, id) = (event.guild_id, event.id);
        let content = div()
            .px(px(24.))
            .pt(px(16.))
            .pb(px(32.))
            .flex()
            .flex_col()
            .children(cover)
            .child(
                div()
                    .mt(px(16.))
                    .relative()
                    .w_full()
                    .child(
                        div()
                            .absolute()
                            .bottom_0()
                            .left_0()
                            .right_0()
                            .h(px(2.))
                            .rounded_full()
                            .bg(rgb(theme::MODAL_BORDER)),
                    )
                    .child(
                        div()
                            .flex()
                            .items_start()
                            .gap(px(32.))
                            .child(tab("Event Info".to_owned(), true))
                            .child(tab(
                                interested.map_or_else(|| "Interested".to_owned(), |count| format!("{count} Interested")),
                                false,
                            )),
                    ),
            )
            .child(
                div()
                    .mt(px(28.))
                    .h(px(20.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(icon("calendar", 20., if active { theme::STATUS_POSITIVE } else { UPCOMING }))
                    .child(
                        div()
                            .text_size(px(14.))
                            .line_height(px(18.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(theme::TEXT_BRIGHT))
                            .child(when),
                    ),
            )
            .child(
                div()
                    .mt(px(16.))
                    .text_size(px(20.))
                    .line_height(px(24.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(theme::TEXT_BRIGHT))
                    .child(event.name.clone()),
            )
            .child(div().mt(px(16.)).flex().flex_col().gap(px(8.)).children(rows))
            .children(description);

        let footer = div()
            .flex_none()
            .px(px(24.))
            .pt(px(16.))
            .flex()
            .items_center()
            .justify_end()
            .gap(px(8.))
            .child(sized_button(
                ElementId::Name(format!("event-detail-more-{id}").into()),
                None,
                "ellipsis",
                true,
                |_: &ClickEvent, _, _| {},
            ))
            .child(sized_button(
                ElementId::Name(format!("event-detail-copy-{id}").into()),
                Some("Copy Link"),
                "link",
                true,
                move |_: &ClickEvent, _, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(event_link(guild_id, id)));
                },
            ))
            .when(!active, |footer| {
                footer.child(sized_button(
                    ElementId::Name(format!("event-detail-interested-{id}").into()),
                    Some("Interested"),
                    "bell",
                    true,
                    |_: &ClickEvent, _, _| {},
                ))
            });

        let height = (window.viewport_size().height - px(155.)).min(px(DETAIL_HEIGHT));
        div()
            .id("event-details")
            .occlude()
            .w(px(DETAIL_WIDTH))
            .h(height)
            .flex()
            .flex_col()
            .overflow_hidden()
            .rounded(px(12.))
            .border_1()
            .border_color(rgb(theme::BORDER_OVERLAY))
            .bg(rgb(theme::BG_PANEL))
            .pt(px(8.))
            .pb(px(16.))
            .child(
                div()
                    .flex_none()
                    .px(px(24.))
                    .h(px(40.))
                    .flex()
                    .items_start()
                    .justify_between()
                    .child(
                        div()
                            .mt(px(16.))
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(px(20.))
                            .line_height(px(24.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(theme::TEXT_BRIGHT))
                            .child(event.name.clone()),
                    )
                    .child(
                        div()
                            .id("close-event-details")
                            .size(px(40.))
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(8.))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(theme::BG_HOVER)))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close(cx)))
                            .child(icon("x", 20., theme::TEXT_MUTED)),
                    ),
            )
            .child(scroll::vertical("event-details-body", &self.detail_scroll, content))
            .child(footer)
            .into_any_element()
    }
}

/// Where the web client sends people to see an event.
fn event_link(guild: Snowflake, event: Snowflake) -> String {
    format!("https://discord.com/events/{guild}/{event}")
}

/// 1108 as "1,108".
fn thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index) % 3 == 0 {
            out.push(',');
        }
        out.push(digit);
    }
    out
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

/// A secondary button: just a glyph, or a glyph and its label. Small ones sit
/// in a card (32 high), large ones under the details (40 high).
fn button(
    id: ElementId,
    label: Option<&'static str>,
    glyph: &'static str,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut gpui_kit::App) + 'static,
) -> AnyElement {
    sized_button(id, label, glyph, false, on_click)
}

fn sized_button(
    id: ElementId,
    label: Option<&'static str>,
    glyph: &'static str,
    large: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut gpui_kit::App) + 'static,
) -> AnyElement {
    let (height, glyph_size, text_size, line) = if large { (40., 20., 16., 20.) } else { (32., 16., 14., 18.) };
    div()
        .id(id)
        .h(px(height))
        .min_w(px(height))
        .px(px(match (label.is_some(), large) {
            (false, _) => 0.,
            (true, true) => 15.,
            (true, false) => 11.,
        }))
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
        .on_click(move |event, window, cx| {
            // The card behind opens its details on a click.
            cx.stop_propagation();
            on_click(event, window, cx);
        })
        .child(icon(glyph, glyph_size, theme::TEXT_BRIGHT))
        .children(label.map(|text| {
            div()
                .text_size(px(text_size))
                .line_height(px(line))
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

        if let Some(detail) = self.detail
            && let Some(event) = events.iter().find(|event| event.id == detail.event)
        {
            let frame = self.details(event, window, cx);
            return modal(frame, Box::new(cx.listener(|this, _: &ClickEvent, _, cx| this.close(cx))));
        }

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
    fn counts_get_thousands_separators() {
        let words: Vec<_> = [0, 7, 999, 1000, 1108, 12_345, 1_234_567].iter().map(|n| thousands(*n)).collect();
        assert_eq!(words, ["0", "7", "999", "1,000", "1,108", "12,345", "1,234,567"]);
    }

    #[test]
    fn the_link_names_the_server_and_event() {
        assert_eq!(event_link(Snowflake(1), Snowflake(2)), "https://discord.com/events/1/2");
    }
}
