//! Turns a channel's messages into the rows the list shows: day dividers
//! and messages, where a message right after one from the same person is
//! drawn without repeating the avatar and name.

use chrono::{FixedOffset, NaiveDate};
use oxicord_model::{Message, MessageType, Snowflake};

/// Messages from one author closer together than this share a header.
const GROUP_WINDOW_SECS: i64 = 7 * 60;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Row {
    /// The first row of a new day.
    Divider(NaiveDate),
    Message { id: Snowflake, grouped: bool },
}

/// `offset` decides where one day ends and the next begins, it should be
/// the user's local offset.
pub fn build_rows<'a>(
    messages: impl IntoIterator<Item = &'a Message>,
    offset: FixedOffset,
) -> Vec<Row> {
    let mut rows = Vec::new();
    let mut previous: Option<&Message> = None;

    for message in messages {
        let day = message.timestamp.with_timezone(&offset).date_naive();
        let same_day =
            previous.is_some_and(|p| p.timestamp.with_timezone(&offset).date_naive() == day);

        if !same_day {
            rows.push(Row::Divider(day));
        }
        let grouped = same_day && previous.is_some_and(|p| shares_header(p, message));
        rows.push(Row::Message {
            id: message.id,
            grouped,
        });
        previous = Some(message);
    }
    rows
}

fn shares_header(previous: &Message, message: &Message) -> bool {
    previous.author.id == message.author.id
        && previous.kind.is_regular()
        && message.kind.is_regular()
        // A reply carries its own header line, so it starts a new group.
        && message.kind != MessageType::Reply
        && message.referenced_message.is_none()
        && (message.timestamp - previous.timestamp).num_seconds() < GROUP_WINDOW_SECS
}

/// What changed between two row lists, as the range of old rows to replace
/// and how many new rows go in their place. Rows shared at the start and end
/// are left alone, so the list keeps its scroll position and its measured
/// heights.
pub fn diff(old: &[Row], new: &[Row]) -> Option<(std::ops::Range<usize>, usize)> {
    if old == new {
        return None;
    }
    let prefix = old.iter().zip(new).take_while(|(a, b)| a == b).count();
    let suffix = old[prefix..]
        .iter()
        .rev()
        .zip(new[prefix..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();

    Some((prefix..old.len() - suffix, new.len() - prefix - suffix))
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Duration, Utc};
    use serde_json::json;

    use super::*;

    fn utc() -> FixedOffset {
        FixedOffset::east_opt(0).unwrap()
    }

    fn message(id: u64, author: u64, minutes: i64) -> Message {
        let base: DateTime<Utc> = "2024-05-01T10:00:00+00:00".parse().unwrap();
        let mut m: Message = serde_json::from_value(json!({
            "id": id.to_string(), "channel_id": "1",
            "author": { "id": author.to_string(), "username": "u" },
            "timestamp": base.to_rfc3339()
        }))
        .unwrap();
        m.timestamp = base + Duration::minutes(minutes);
        m
    }

    fn row(id: u64, grouped: bool) -> Row {
        Row::Message {
            id: Snowflake(id),
            grouped,
        }
    }

    #[test]
    fn same_author_within_the_window_shares_a_header() {
        let messages = [
            message(1, 1, 0),
            message(2, 1, 3),
            message(3, 1, 20),
            message(4, 2, 21),
        ];
        assert_eq!(
            build_rows(&messages, utc()),
            [
                Row::Divider("2024-05-01".parse().unwrap()),
                row(1, false),
                row(2, true),
                row(3, false),
                row(4, false),
            ]
        );
    }

    #[test]
    fn a_new_day_gets_a_divider_and_breaks_the_group() {
        let messages = [message(1, 1, 0), message(2, 1, 14 * 60 + 1)];
        let rows = build_rows(&messages, utc());
        assert_eq!(rows.len(), 4);
        assert_eq!(rows[2], Row::Divider("2024-05-02".parse().unwrap()));
        assert_eq!(rows[3], row(2, false));
    }

    #[test]
    fn the_local_offset_moves_the_day_boundary() {
        // 09:59 and 10:01 UTC are the same day in UTC, but at +14 they
        // straddle midnight (23:59 and 00:01).
        let messages = [message(1, 1, -1), message(2, 1, 1)];
        let east = FixedOffset::east_opt(14 * 3600).unwrap();
        assert_eq!(build_rows(&messages, utc()).len(), 3);
        assert_eq!(build_rows(&messages, east).len(), 4);
    }

    #[test]
    fn a_reply_starts_its_own_group() {
        let mut reply = message(2, 1, 1);
        reply.kind = MessageType::Reply;
        let rows = build_rows(&[message(1, 1, 0), reply], utc());
        assert_eq!(rows[2], row(2, false));
    }

    #[test]
    fn system_messages_never_share_a_header() {
        let mut joined = message(2, 1, 1);
        joined.kind = MessageType::UserJoin;
        let rows = build_rows(&[message(1, 1, 0), joined], utc());
        assert_eq!(rows[2], row(2, false));
    }

    #[test]
    fn diff_finds_a_prepend() {
        let old = [row(3, false), row(4, true)];
        let new = [row(1, false), row(2, true), row(3, true), row(4, true)];
        // The old first row changed (it now follows another), so it is
        // replaced along with the new ones.
        assert_eq!(diff(&old, &new), Some((0..1, 3)));
    }

    #[test]
    fn diff_finds_an_append_and_an_edit_in_place() {
        let old = [row(1, false), row(2, true)];
        let appended = [row(1, false), row(2, true), row(3, true)];
        assert_eq!(diff(&old, &appended), Some((2..2, 1)));
        assert_eq!(diff(&old, &[row(1, false)]), Some((1..2, 0)));
        assert_eq!(diff(&old, &old), None);
    }
}
