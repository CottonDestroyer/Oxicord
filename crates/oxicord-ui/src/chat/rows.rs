//! Turns a channel's messages into the rows the list shows: day dividers
//! and messages, where a message right after one from the same person is
//! drawn without repeating the avatar and name.

use std::collections::HashSet;

use chrono::{FixedOffset, NaiveDate};
use oxicord_model::{Message, MessageType, Snowflake};

/// Messages from one author closer together than this share a header.
const GROUP_WINDOW_SECS: i64 = 7 * 60;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Row {
    /// Who the chat is with, above the first message once all history is in.
    Intro,
    /// The first row of a new day.
    Divider(NaiveDate),
    /// Where the messages we have not read begin.
    NewMessages,
    /// A run of messages from blocked people folded into one bar. `first` is
    /// the oldest of them and names the run; `shown` says the reader opened
    /// it, in which case the messages follow the bar.
    Blocked { first: Snowflake, count: usize, shown: bool },
    Message { id: Snowflake, grouped: bool },
}

/// `offset` decides where one day ends and the next begins, it should be
/// the user's local offset. Messages for which `hidden` is true collapse into
/// a [`Row::Blocked`] bar unless the run's first id is in `shown`.
pub fn build_rows<'a>(
    messages: impl IntoIterator<Item = &'a Message>,
    offset: FixedOffset,
    unread_after: Option<Snowflake>,
    hidden: &dyn Fn(&Message) -> bool,
    shown: &HashSet<Snowflake>,
) -> Vec<Row> {
    let mut rows: Vec<Row> = Vec::new();
    // The last message drawn with its own row, which the next one may join.
    let mut drawn: Option<&Message> = None;
    let mut previous: Option<&Message> = None;
    // The bar of the run being collected, as its index in `rows`.
    let mut run: Option<usize> = None;
    let mut marked = false;

    for message in messages {
        let day = message.timestamp.with_timezone(&offset).date_naive();
        let same_day =
            previous.is_some_and(|p| p.timestamp.with_timezone(&offset).date_naive() == day);

        if !same_day {
            rows.push(Row::Divider(day));
            run = None;
        }
        // The marker goes before the first message newer than the read point,
        // and only when something older is shown above it.
        let starts_unread = !marked && unread_after.is_some_and(|read| message.id > read);
        if starts_unread {
            marked = true;
            run = None;
            if previous.is_some() {
                rows.push(Row::NewMessages);
            }
        }
        previous = Some(message);

        if hidden(message) {
            match run {
                Some(at) => {
                    if let Row::Blocked { count, .. } = &mut rows[at] {
                        *count += 1;
                    }
                }
                None => {
                    rows.push(Row::Blocked {
                        first: message.id,
                        count: 1,
                        shown: shown.contains(&message.id),
                    });
                    run = Some(rows.len() - 1);
                    drawn = None;
                }
            }
            let Some(Row::Blocked { shown: open, .. }) = run.map(|at| &rows[at]) else { continue };
            if !*open {
                continue;
            }
        } else {
            run = None;
        }

        let grouped = !starts_unread && same_day && drawn.is_some_and(|p| shares_header(p, message));
        rows.push(Row::Message {
            id: message.id,
            grouped,
        });
        drawn = Some(message);
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
            build_rows(&messages, utc(), None, &|_| false, &HashSet::new()),
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
        let rows = build_rows(&messages, utc(), None, &|_| false, &HashSet::new());
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
        assert_eq!(build_rows(&messages, utc(), None, &|_| false, &HashSet::new()).len(), 3);
        assert_eq!(build_rows(&messages, east, None, &|_| false, &HashSet::new()).len(), 4);
    }

    #[test]
    fn a_reply_starts_its_own_group() {
        let mut reply = message(2, 1, 1);
        reply.kind = MessageType::Reply;
        let rows = build_rows(&[message(1, 1, 0), reply], utc(), None, &|_| false, &HashSet::new());
        assert_eq!(rows[2], row(2, false));
    }

    #[test]
    fn system_messages_never_share_a_header() {
        let mut joined = message(2, 1, 1);
        joined.kind = MessageType::UserJoin;
        let rows = build_rows(&[message(1, 1, 0), joined], utc(), None, &|_| false, &HashSet::new());
        assert_eq!(rows[2], row(2, false));
    }

    #[test]
    fn new_marker_goes_before_the_first_unread_message_and_breaks_its_group() {
        let messages = [message(1, 1, 0), message(2, 1, 1), message(3, 1, 2)];
        let rows = build_rows(&messages, utc(), Some(Snowflake(2)), &|_| false, &HashSet::new());
        assert_eq!(
            rows,
            [
                Row::Divider("2024-05-01".parse().unwrap()),
                row(1, false),
                row(2, true),
                Row::NewMessages,
                row(3, false),
            ]
        );
    }

    #[test]
    fn no_marker_when_everything_is_read_or_nothing_is_above_it() {
        let messages = [message(5, 1, 0), message(6, 1, 1)];
        let read = build_rows(&messages, utc(), Some(Snowflake(6)), &|_| false, &HashSet::new());
        assert!(!read.contains(&Row::NewMessages));

        // Everything shown is newer than the read point, so there is no
        // older message to put the marker under.
        let all_unread = build_rows(&messages, utc(), Some(Snowflake(1)), &|_| false, &HashSet::new());
        assert!(!all_unread.contains(&Row::NewMessages));
    }

    fn blocked(first: u64, count: usize, shown: bool) -> Row {
        Row::Blocked { first: Snowflake(first), count, shown }
    }

    #[test]
    fn consecutive_blocked_messages_fold_into_one_bar() {
        let messages = [message(1, 1, 0), message(2, 9, 1), message(3, 9, 2), message(4, 1, 3)];
        let rows = build_rows(&messages, utc(), None, &|m| m.author.id == Snowflake(9), &HashSet::new());
        assert_eq!(
            rows,
            [
                Row::Divider("2024-05-01".parse().unwrap()),
                row(1, false),
                blocked(2, 2, false),
                // The bar sits between them, so this one starts a new group.
                row(4, false),
            ]
        );
    }

    #[test]
    fn an_opened_run_keeps_its_bar_and_lists_the_messages() {
        let messages = [message(1, 9, 0), message(2, 9, 1)];
        let shown = HashSet::from([Snowflake(1)]);
        let rows = build_rows(&messages, utc(), None, &|_| true, &shown);
        assert_eq!(
            rows,
            [
                Row::Divider("2024-05-01".parse().unwrap()),
                blocked(1, 2, true),
                row(1, false),
                row(2, true),
            ]
        );
    }

    #[test]
    fn a_visible_message_splits_two_blocked_runs() {
        let messages = [message(1, 9, 0), message(2, 1, 1), message(3, 9, 2)];
        let rows = build_rows(&messages, utc(), None, &|m| m.author.id == Snowflake(9), &HashSet::new());
        assert_eq!(rows.iter().filter(|r| matches!(r, Row::Blocked { .. })).count(), 2);
        let messages = [message(1, 9, 0), message(2, 1, 1), message(3, 9, 2), message(4, 9, 3)];
        let rows = build_rows(&messages, utc(), None, &|m| m.author.id == Snowflake(9), &HashSet::new());
        assert_eq!(rows[1], blocked(1, 1, false));
        assert_eq!(rows[3], blocked(3, 2, false));
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
