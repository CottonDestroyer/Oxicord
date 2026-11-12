//! Scheduled events a server lists in its channel sidebar. They arrive inside
//! the guild payload and are kept up to date by `GUILD_SCHEDULED_EVENT_*`.

use chrono::{DateTime, Utc};
use serde::Deserialize;

use crate::Snowflake;

/// Where an event is in its life.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventStatus {
    Scheduled,
    Active,
    Completed,
    Canceled,
}

/// Where an event takes place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventPlace {
    Stage,
    Voice,
    External,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ScheduledEvent {
    pub id: Snowflake,
    pub guild_id: Snowflake,
    #[serde(default)]
    pub channel_id: Option<Snowflake>,
    #[serde(default)]
    pub creator_id: Option<Snowflake>,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub scheduled_start_time: DateTime<Utc>,
    #[serde(default)]
    pub scheduled_end_time: Option<DateTime<Utc>>,
    #[serde(default)]
    status: u8,
    #[serde(default)]
    entity_type: u8,
    #[serde(default)]
    entity_metadata: Option<EntityMetadata>,
    /// The cover picture's hash.
    #[serde(default)]
    pub image: Option<String>,
    /// How many people are interested, when the server sent it.
    #[serde(default)]
    pub user_count: Option<u64>,
    /// Set when the event repeats.
    #[serde(default)]
    pub recurrence_rule: Option<serde_json::Value>,
}

/// The answer to `GET .../scheduled-events/:id/users/counts`.
#[derive(Clone, Copy, Debug, Default, Deserialize)]
pub struct EventUserCounts {
    /// How many people are interested.
    #[serde(default)]
    pub guild_scheduled_event_count: u64,
}

/// `GUILD_SCHEDULED_EVENT_USER_ADD` and `_USER_REMOVE`: somebody answered an
/// event, or took their answer back.
#[derive(Clone, Copy, Debug, Deserialize)]
pub struct EventUserChange {
    pub user_id: Snowflake,
    pub guild_scheduled_event_id: Snowflake,
    /// 1 interested, 0 not interested. Left out, it means interested.
    #[serde(default)]
    pub response: Option<u8>,
}

impl EventUserChange {
    /// Whether this moves the interested total.
    pub fn is_interested(&self) -> bool {
        self.response.unwrap_or(1) == 1
    }
}

/// One entry of `GET /users/@me/scheduled-events`: an event we answered.
#[derive(Clone, Copy, Debug, Deserialize)]
pub struct EventRsvp {
    pub guild_scheduled_event_id: Snowflake,
    /// 1 is interested, 0 is not interested.
    #[serde(default)]
    pub response: u8,
}

impl EventRsvp {
    pub fn is_interested(&self) -> bool {
        self.response == 1
    }
}

/// Where a new event will be held.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NewPlace {
    Stage(Snowflake),
    Voice(Snowflake),
    /// A link or an address, which needs an end time.
    External(String),
}

/// An event the Create Event form is about to send.
#[derive(Clone, Debug)]
pub struct NewEvent {
    pub name: String,
    pub description: String,
    pub place: NewPlace,
    pub start: DateTime<Utc>,
    pub end: Option<DateTime<Utc>>,
}

impl NewEvent {
    /// The first thing wrong with the form, worded like the web client, or
    /// `None` when it can be sent.
    pub fn problem(&self, now: DateTime<Utc>) -> Option<&'static str> {
        if self.name.trim().is_empty() {
            return Some("Event topic is required.");
        }
        if matches!(&self.place, NewPlace::External(place) if place.trim().is_empty()) {
            return Some("A location is required.");
        }
        if self.start <= now {
            return Some("The start time must be in the future.");
        }
        match (&self.place, self.end) {
            (NewPlace::External(_), None) => Some("End date and time are required."),
            (_, Some(end)) if end <= self.start => Some("The end time must be after the start time."),
            _ => None,
        }
    }

    /// The body of `POST /guilds/:id/scheduled-events`. Everyone in the
    /// server may see the event (privacy level 2, the only one).
    pub fn body(&self) -> serde_json::Value {
        let mut body = serde_json::json!({
            "name": self.name.trim(),
            "privacy_level": 2,
            "scheduled_start_time": self.start.to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            "recurrence_rule": null,
        });
        let fields = body.as_object_mut().expect("the body is an object");
        let (entity_type, channel, location) = match &self.place {
            NewPlace::Stage(channel) => (1, Some(channel), None),
            NewPlace::Voice(channel) => (2, Some(channel), None),
            NewPlace::External(location) => (3, None, Some(location.trim())),
        };
        fields.insert("entity_type".into(), entity_type.into());
        fields.insert("channel_id".into(), channel.map_or(serde_json::Value::Null, |c| c.to_string().into()));
        fields.insert(
            "entity_metadata".into(),
            location.map_or(serde_json::Value::Null, |location| serde_json::json!({ "location": location })),
        );
        if let Some(end) = self.end {
            fields.insert(
                "scheduled_end_time".into(),
                end.to_rfc3339_opts(chrono::SecondsFormat::Millis, true).into(),
            );
        }
        let description = self.description.trim();
        if !description.is_empty() {
            fields.insert("description".into(), description.into());
        }
        body
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
struct EntityMetadata {
    #[serde(default)]
    location: Option<String>,
}

impl ScheduledEvent {
    pub fn status(&self) -> EventStatus {
        match self.status {
            2 => EventStatus::Active,
            3 => EventStatus::Completed,
            4 => EventStatus::Canceled,
            _ => EventStatus::Scheduled,
        }
    }

    pub fn place(&self) -> EventPlace {
        match self.entity_type {
            1 => EventPlace::Stage,
            2 => EventPlace::Voice,
            _ => EventPlace::External,
        }
    }

    /// The place text of an event held outside Discord.
    pub fn location(&self) -> Option<&str> {
        self.entity_metadata
            .as_ref()
            .and_then(|meta| meta.location.as_deref())
            .filter(|location| !location.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_external_event_reads_its_place_and_status() {
        let event: ScheduledEvent = serde_json::from_str(
            r#"{"id":"7","guild_id":"1","name":"Field Evaluation","status":2,"entity_type":3,
                "entity_metadata":{"location":"play.example.com"},
                "scheduled_start_time":"2026-11-10T19:00:00+00:00","user_count":1108}"#,
        )
        .unwrap();
        assert_eq!(event.status(), EventStatus::Active);
        assert_eq!(event.place(), EventPlace::External);
        assert_eq!(event.location(), Some("play.example.com"));
        assert_eq!(event.user_count, Some(1108));
    }

    #[test]
    fn the_captured_list_parses() {
        let text = include_str!("../../../fixtures/scheduled-events.json");
        let events: Vec<ScheduledEvent> = serde_json::from_str(text).unwrap();
        assert_eq!(events.len(), 4);
        assert_eq!(events[0].name, "Upsilon - 11 Field Evaluation");
        assert_eq!(events[0].status(), EventStatus::Active);
        assert_eq!(events[0].location(), Some("play.scproleplay.com"));
        assert!(events[0].image.is_some());
        assert_eq!(events[1].status(), EventStatus::Scheduled);
        assert!(events[1].scheduled_end_time.is_some());
    }

    #[test]
    fn the_counts_answer_names_the_interested_total() {
        let counts: EventUserCounts = serde_json::from_str(
            r#"{"guild_scheduled_event_count":1108,"guild_scheduled_event_exception_counts":{}}"#,
        )
        .unwrap();
        assert_eq!(counts.guild_scheduled_event_count, 1108);
    }

    #[test]
    fn an_rsvp_reads_the_event_and_the_answer() {
        let rsvps: Vec<EventRsvp> = serde_json::from_str(
            r#"[{"guild_scheduled_event_id":"42","user_id":"7","response":1},
                {"guild_scheduled_event_id":"43","user_id":"7","response":0}]"#,
        )
        .unwrap();
        assert_eq!(rsvps[0].guild_scheduled_event_id, Snowflake(42));
        assert!(rsvps[0].is_interested());
        assert!(!rsvps[1].is_interested());
    }

    fn time(hour: u32) -> DateTime<Utc> {
        use chrono::TimeZone;
        Utc.with_ymd_and_hms(2026, 11, 20, hour, 0, 0).unwrap()
    }

    fn draft(place: NewPlace, end: Option<DateTime<Utc>>) -> NewEvent {
        NewEvent { name: " Raid night ".into(), description: String::new(), place, start: time(19), end }
    }

    #[test]
    fn a_voice_event_posts_its_channel_and_no_location() {
        let body = draft(NewPlace::Voice(Snowflake(5)), None).body();
        assert_eq!(body["entity_type"], 2);
        assert_eq!(body["channel_id"], "5");
        assert!(body["entity_metadata"].is_null());
        assert_eq!(body["name"], "Raid night");
        assert_eq!(body["privacy_level"], 2);
        assert_eq!(body["scheduled_start_time"], "2026-11-20T19:00:00.000Z");
        assert!(body.get("scheduled_end_time").is_none());
        assert!(body.get("description").is_none());
    }

    #[test]
    fn an_external_event_posts_its_location_end_and_description() {
        let mut event = draft(NewPlace::External(" play.example.com ".into()), Some(time(21)));
        event.description = "bring snacks".into();
        let body = event.body();
        assert_eq!(body["entity_type"], 3);
        assert!(body["channel_id"].is_null());
        assert_eq!(body["entity_metadata"]["location"], "play.example.com");
        assert_eq!(body["scheduled_end_time"], "2026-11-20T21:00:00.000Z");
        assert_eq!(body["description"], "bring snacks");
        assert_eq!(draft(NewPlace::Stage(Snowflake(9)), None).body()["entity_type"], 1);
    }

    #[test]
    fn the_form_names_what_is_missing_in_the_web_clients_words() {
        let now = time(12);
        let ok = draft(NewPlace::Voice(Snowflake(5)), None);
        assert_eq!(ok.problem(now), None);

        let mut nameless = ok.clone();
        nameless.name = "  ".into();
        assert_eq!(nameless.problem(now), Some("Event topic is required."));

        assert_eq!(ok.problem(time(20)), Some("The start time must be in the future."));
        assert_eq!(
            draft(NewPlace::Voice(Snowflake(5)), Some(time(18))).problem(now),
            Some("The end time must be after the start time.")
        );
        assert_eq!(
            draft(NewPlace::External("".into()), Some(time(21))).problem(now),
            Some("A location is required.")
        );
        assert_eq!(
            draft(NewPlace::External("here".into()), None).problem(now),
            Some("End date and time are required.")
        );
        assert_eq!(draft(NewPlace::External("here".into()), Some(time(21))).problem(now), None);
    }

    #[test]
    fn a_voice_event_has_no_location() {
        let event: ScheduledEvent = serde_json::from_str(
            r#"{"id":"7","guild_id":"1","channel_id":"3","status":1,"entity_type":2,
                "entity_metadata":null,"scheduled_start_time":"2026-11-10T19:00:00+00:00"}"#,
        )
        .unwrap();
        assert_eq!(event.status(), EventStatus::Scheduled);
        assert_eq!(event.place(), EventPlace::Voice);
        assert_eq!(event.location(), None);
    }
}
