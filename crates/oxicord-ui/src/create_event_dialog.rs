//! The Create Event wizard behind the server menu's "Create Event" row. The
//! web client asks three things in turn: where the event is held, what it is
//! and when, then a review. Sending it is gated like every account write:
//! built and unit tested, never sent from the real account.

use chrono::{DateTime, Local, NaiveDate, NaiveTime, TimeZone, Utc};
use gpui_kit::{
    AnyElement, AppContext as _, ClickEvent, Context, Entity, EventEmitter, FontWeight, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _, Window, div, px, rgb,
    component::input::{Input, InputState},
};
use oxicord_model::{ChannelType, NewEvent, NewPlace, Snowflake};

use crate::{
    backend::{Backend, Command},
    icons::icon,
    profile::modal,
    session::Session,
    theme,
};

const CARD_WIDTH: f32 = 540.;

pub struct CloseCreateEvent;

impl EventEmitter<CloseCreateEvent> for CreateEventDialog {}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Step {
    Place,
    Info,
    Review,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Voice,
    Stage,
    Elsewhere,
}

pub struct CreateEventDialog {
    session: Entity<Session>,
    backend: Backend,
    guild: Option<Snowflake>,
    step: Step,
    kind: Option<Kind>,
    channel: Option<Snowflake>,
    location: Entity<InputState>,
    topic: Entity<InputState>,
    start_date: Entity<InputState>,
    start_time: Entity<InputState>,
    end_date: Entity<InputState>,
    end_time: Entity<InputState>,
    description: Entity<InputState>,
    with_end: bool,
    problem: Option<String>,
}

impl CreateEventDialog {
    pub fn new(session: Entity<Session>, backend: Backend, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let field = |window: &mut Window, cx: &mut Context<Self>, hint: &'static str| {
            cx.new(|cx| InputState::new(window, cx).placeholder(hint))
        };
        Self {
            session,
            backend,
            guild: None,
            step: Step::Place,
            kind: None,
            channel: None,
            location: field(window, cx, "Add a location, link, or something."),
            topic: field(window, cx, "What's your event?"),
            start_date: field(window, cx, "YYYY-MM-DD"),
            start_time: field(window, cx, "7:00 PM"),
            end_date: field(window, cx, "YYYY-MM-DD"),
            end_time: field(window, cx, "9:00 PM"),
            description: field(window, cx, "Tell people a little more about your event."),
            with_end: false,
            problem: None,
        }
    }

    pub fn opened(&mut self, guild: Snowflake, window: &mut Window, cx: &mut Context<Self>) {
        self.guild = Some(guild);
        self.step = Step::Place;
        self.kind = None;
        self.channel = None;
        self.with_end = false;
        self.problem = None;
        for input in self.inputs() {
            input.update(cx, |input, cx| input.set_value("", window, cx));
        }
        cx.notify();
    }

    fn inputs(&self) -> [Entity<InputState>; 7] {
        [
            self.location.clone(),
            self.topic.clone(),
            self.start_date.clone(),
            self.start_time.clone(),
            self.end_date.clone(),
            self.end_time.clone(),
            self.description.clone(),
        ]
    }

    /// The server's voice or stage channels that can hold the event.
    fn channels(&self, kind: ChannelType, cx: &Context<Self>) -> Vec<(Snowflake, String)> {
        let Some(guild) = self.guild else { return Vec::new() };
        let session = self.session.read(cx);
        session
            .store
            .channel_groups(guild)
            .into_iter()
            .flat_map(|group| group.channels)
            .filter(|channel| channel.kind == kind)
            .map(|channel| (channel.id, channel.name.clone().unwrap_or_default()))
            .collect()
    }

    fn place(&self, cx: &Context<Self>) -> Option<NewPlace> {
        match self.kind? {
            Kind::Voice => self.channel.map(NewPlace::Voice),
            Kind::Stage => self.channel.map(NewPlace::Stage),
            Kind::Elsewhere => Some(NewPlace::External(self.location.read(cx).value().to_string())),
        }
    }

    /// The form as the server would receive it, or what is wrong with it.
    fn draft(&self, cx: &Context<Self>) -> Result<NewEvent, String> {
        let place = self.place(cx).ok_or("Select a channel")?;
        let text = |input: &Entity<InputState>| input.read(cx).value().to_string();
        let start = when(&Local, &text(&self.start_date), &text(&self.start_time)).ok_or("Start date and time are required.")?;
        let end = if self.with_end {
            Some(when(&Local, &text(&self.end_date), &text(&self.end_time)).ok_or("End date and time are required.")?)
        } else {
            None
        };
        let event = NewEvent { name: text(&self.topic), description: text(&self.description), place, start, end };
        match event.problem(Utc::now()) {
            Some(problem) => Err(problem.to_owned()),
            None => Ok(event),
        }
    }

    fn next(&mut self, cx: &mut Context<Self>) {
        self.problem = match self.step {
            Step::Place => match self.place(cx) {
                None => Some("Select a channel".to_owned()),
                Some(NewPlace::External(text)) if text.trim().is_empty() => Some("A location is required.".to_owned()),
                Some(_) => None,
            },
            Step::Info => self.draft(cx).err(),
            Step::Review => None,
        };
        if self.problem.is_none() {
            self.step = match self.step {
                Step::Place => Step::Info,
                Step::Info | Step::Review => Step::Review,
            };
        }
        cx.notify();
    }

    fn back(&mut self, cx: &mut Context<Self>) {
        self.problem = None;
        self.step = match self.step {
            Step::Place | Step::Info => Step::Place,
            Step::Review => Step::Info,
        };
        cx.notify();
    }

    fn create(&mut self, cx: &mut Context<Self>) {
        let Some(guild) = self.guild else { return };
        match self.draft(cx) {
            Ok(event) => {
                self.backend.send(Command::CreateEvent { guild, event });
                cx.emit(CloseCreateEvent);
            }
            Err(problem) => {
                self.problem = Some(problem);
                cx.notify();
            }
        }
    }

    fn pick_kind(&mut self, kind: Kind, cx: &mut Context<Self>) {
        if self.kind != Some(kind) {
            self.channel = None;
        }
        self.kind = Some(kind);
        self.problem = None;
        cx.notify();
    }

    fn render_place(&self, cx: &mut Context<Self>) -> AnyElement {
        let options = [
            (Kind::Voice, "volume-2", "Voice Channel", "Hang out with voice, video, screenshare, and Go Live."),
            (Kind::Stage, "mic", "Stage Channel", "Great for larger community audio events."),
            (Kind::Elsewhere, "location-pin", "Somewhere Else", "Text channel, external link, or in-person location."),
        ];
        let mut body = div().flex().flex_col().gap(px(8.));
        for (kind, glyph, title, hint) in options {
            let chosen = self.kind == Some(kind);
            body = body.child(
                div()
                    .id(("event-kind", kind as usize))
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .p(px(12.))
                    .rounded(px(8.))
                    .border_1()
                    .border_color(rgb(if chosen { theme::ACCENT } else { theme::MODAL_BORDER }))
                    .bg(rgb(if chosen { theme::BG_ACTIVE } else { theme::BG_CARD }))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(theme::BG_ACTIVE)))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.pick_kind(kind, cx)))
                    .child(icon(glyph, 24., theme::TEXT_MUTED))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(div().text_size(px(16.)).font_weight(FontWeight::SEMIBOLD).text_color(rgb(theme::TEXT_BRIGHT)).child(title))
                            .child(div().text_size(px(14.)).text_color(rgb(theme::TEXT_MUTED)).child(hint)),
                    ),
            );
        }
        match self.kind {
            Some(Kind::Elsewhere) => {
                body = body.child(label("Enter a location")).child(Input::new(&self.location));
            }
            Some(kind) => {
                let channels = self.channels(if kind == Kind::Voice { ChannelType::GuildVoice } else { ChannelType::GuildStageVoice }, cx);
                body = body.child(label("Select a channel"));
                if channels.is_empty() {
                    body = body.child(div().text_size(px(14.)).text_color(rgb(theme::TEXT_MUTED)).child("There are no available channels."));
                }
                for (index, (id, name)) in channels.into_iter().enumerate() {
                    let chosen = self.channel == Some(id);
                    body = body.child(
                        div()
                            .id(("event-channel", index))
                            .h(px(36.))
                            .px(px(12.))
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .rounded(px(4.))
                            .bg(rgb(if chosen { theme::BG_ACTIVE } else { theme::BG_CARD }))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(theme::BG_ACTIVE)))
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.channel = Some(id);
                                this.problem = None;
                                cx.notify();
                            }))
                            .child(icon(if kind == Kind::Voice { "volume-2" } else { "mic" }, 16., theme::TEXT_MUTED))
                            .child(div().text_size(px(16.)).text_color(rgb(theme::TEXT_BRIGHT)).child(name))
                            .child(div().flex_1())
                            .children(chosen.then(|| icon("check", 16., theme::TEXT_BRIGHT))),
                    );
                }
            }
            None => {}
        }
        body.into_any_element()
    }

    fn render_info(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut body = div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(label("Event Topic"))
            .child(Input::new(&self.topic))
            .child(
                div()
                    .flex()
                    .gap(px(8.))
                    .child(div().flex_1().flex().flex_col().gap(px(8.)).child(label("Start Date")).child(Input::new(&self.start_date)))
                    .child(div().flex_1().flex().flex_col().gap(px(8.)).child(label("Start Time")).child(Input::new(&self.start_time))),
            );
        if self.with_end {
            body = body.child(
                div()
                    .flex()
                    .gap(px(8.))
                    .child(div().flex_1().flex().flex_col().gap(px(8.)).child(label("End Date")).child(Input::new(&self.end_date)))
                    .child(div().flex_1().flex().flex_col().gap(px(8.)).child(label("End Time")).child(Input::new(&self.end_time))),
            );
        }
        body.child(
            div()
                .id("event-toggle-end")
                .text_size(px(14.))
                .text_color(rgb(theme::ACCENT))
                .cursor_pointer()
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.with_end = !this.with_end;
                    cx.notify();
                }))
                .child(if self.with_end { "Remove end date and time" } else { "Add end date and time" }),
        )
        .child(label("Event Description"))
        .child(Input::new(&self.description))
        .into_any_element()
    }

    fn render_review(&self, cx: &mut Context<Self>) -> AnyElement {
        let Ok(event) = self.draft(cx) else { return div().into_any_element() };
        let place = match &event.place {
            NewPlace::Voice(channel) | NewPlace::Stage(channel) => self
                .session
                .read(cx)
                .store
                .channel(*channel)
                .and_then(|c| c.name.clone())
                .unwrap_or_default(),
            NewPlace::External(location) => location.trim().to_owned(),
        };
        let when = event.start.with_timezone(&Local).format("%a, %b %-d, %-I:%M %p").to_string();
        div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(div().text_size(px(14.)).text_color(rgb(theme::TEXT_MUTED)).child("Here's a preview of your event."))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .p(px(16.))
                    .rounded(px(8.))
                    .border_1()
                    .border_color(rgb(theme::MODAL_BORDER))
                    .bg(rgb(theme::BG_CARD))
                    .child(div().flex().items_center().gap(px(6.)).child(icon("calendar", 16., theme::TEXT_MUTED)).child(div().text_size(px(12.)).font_weight(FontWeight::BOLD).text_color(rgb(theme::TEXT_MUTED)).child(when)))
                    .child(div().text_size(px(16.)).font_weight(FontWeight::SEMIBOLD).text_color(rgb(theme::TEXT_BRIGHT)).child(event.name.trim().to_owned()))
                    .child(div().text_size(px(14.)).text_color(rgb(theme::TEXT_MUTED)).child(place)),
            )
            .into_any_element()
    }
}

fn label(text: &'static str) -> impl IntoElement {
    div().text_size(px(12.)).font_weight(FontWeight::BOLD).text_color(rgb(theme::TEXT_MUTED)).child(text.to_uppercase())
}

/// A date and a time typed by hand, read in the given zone. The time takes a
/// 12 hour clock ("7:00 PM") or a 24 hour one ("19:00").
fn when<Tz: TimeZone>(zone: &Tz, date: &str, time: &str) -> Option<DateTime<Utc>> {
    let date = NaiveDate::parse_from_str(date.trim(), "%Y-%m-%d").ok()?;
    let time = time.trim().to_uppercase();
    let time = ["%I:%M %p", "%I:%M%p", "%I %p", "%H:%M"]
        .iter()
        .find_map(|format| NaiveTime::parse_from_str(&time, format).ok())?;
    zone.from_local_datetime(&date.and_time(time)).earliest().map(|at| at.with_timezone(&Utc))
}

impl Render for CreateEventDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (title, subtitle) = match self.step {
            Step::Place => ("Where is your event?", "So no one gets lost on where to go."),
            Step::Info => ("What's your event about?", "Tell people a little more about your event."),
            Step::Review => ("Review", "Here's a preview of your event."),
        };
        let body = match self.step {
            Step::Place => self.render_place(cx),
            Step::Info => self.render_info(cx),
            Step::Review => self.render_review(cx),
        };
        let button = |id: &'static str, text: &'static str, primary: bool| {
            div()
                .id(id)
                .h(px(38.))
                .px(px(16.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(theme::RADIUS))
                .bg(rgb(if primary { theme::ACCENT } else { theme::SECONDARY }))
                .text_color(rgb(theme::TEXT_BRIGHT))
                .cursor_pointer()
                .hover(|s| s.opacity(0.85))
                .child(text)
        };
        let mut footer = div().flex().items_center().justify_end().gap(px(8.));
        if self.step != Step::Place {
            footer = footer.child(button("event-back", "Back", false).on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.back(cx))));
        }
        footer = footer.child(match self.step {
            Step::Review => button("event-create", "Create Event", true)
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.create(cx)))
                .into_any_element(),
            _ => button("event-next", "Next", true)
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.next(cx)))
                .into_any_element(),
        });
        let card = div()
            .id("create-event-card")
            .occlude()
            .w(px(CARD_WIDTH))
            .p(px(24.))
            .flex()
            .flex_col()
            .gap(px(16.))
            .rounded(px(12.))
            .border_1()
            .border_color(rgb(theme::MODAL_BORDER))
            .bg(rgb(theme::BG_PANEL))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child(div().text_size(px(20.)).font_weight(FontWeight::BOLD).text_color(rgb(theme::TEXT_BRIGHT)).child(title))
                    .child(div().text_size(px(14.)).text_color(rgb(theme::TEXT_MUTED)).child(subtitle)),
            )
            .child(body)
            .children(self.problem.clone().map(|problem| {
                div().text_size(px(14.)).text_color(rgb(theme::DANGER)).child(problem)
            }))
            .child(footer);
        modal(card.into_any_element(), Box::new(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(CloseCreateEvent))))
    }
}

#[cfg(test)]
mod tests {
    use chrono::FixedOffset;

    use super::*;

    fn east(hours: i32) -> FixedOffset {
        FixedOffset::east_opt(hours * 3600).unwrap()
    }

    #[test]
    fn a_typed_date_and_time_are_read_in_the_zone_given() {
        let at = when(&east(2), "2026-11-20", "7:00 PM").unwrap();
        assert_eq!(at.to_rfc3339(), "2026-11-20T17:00:00+00:00");
        assert_eq!(when(&east(0), " 2026-11-20 ", "19:30").unwrap().to_rfc3339(), "2026-11-20T19:30:00+00:00");
        assert_eq!(when(&east(0), "2026-11-20", "7:05 pm").unwrap().to_rfc3339(), "2026-11-20T19:05:00+00:00");
        assert_eq!(when(&east(0), "2026-11-20", "12:00 AM").unwrap().to_rfc3339(), "2026-11-20T00:00:00+00:00");
    }

    #[test]
    fn nonsense_does_not_parse() {
        assert!(when(&east(0), "tomorrow", "7:00 PM").is_none());
        assert!(when(&east(0), "2026-11-20", "soon").is_none());
        assert!(when(&east(0), "2026-13-40", "7:00 PM").is_none());
        assert!(when(&east(0), "", "").is_none());
    }
}
