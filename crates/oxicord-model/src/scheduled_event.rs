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
