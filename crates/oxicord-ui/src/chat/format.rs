//! Dates and times as the message list shows them.

use chrono::{DateTime, FixedOffset, NaiveDate, Utc};

/// "Today at 14:05", "Yesterday at 14:05", or the full date beyond that.
pub fn message_time(at: DateTime<Utc>, now: DateTime<Utc>, offset: FixedOffset) -> String {
    let local = at.with_timezone(&offset);
    let today = now.with_timezone(&offset).date_naive();
    let day = local.date_naive();
    let time = local.format("%H:%M");

    if day == today {
        format!("Today at {time}")
    } else if today.pred_opt() == Some(day) {
        format!("Yesterday at {time}")
    } else {
        format!("{} {time}", local.format("%d/%m/%Y"))
    }
}

pub fn day_label(day: NaiveDate) -> String {
    day.format("%B %-d, %Y").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(text: &str) -> DateTime<Utc> {
        text.parse().unwrap()
    }

    fn utc() -> FixedOffset {
        FixedOffset::east_opt(0).unwrap()
    }

    #[test]
    fn recent_messages_are_relative() {
        let now = at("2024-05-10T12:00:00Z");
        assert_eq!(message_time(at("2024-05-10T09:05:00Z"), now, utc()), "Today at 09:05");
        assert_eq!(message_time(at("2024-05-09T23:59:00Z"), now, utc()), "Yesterday at 23:59");
        assert_eq!(message_time(at("2024-05-01T08:00:00Z"), now, utc()), "01/05/2024 08:00");
    }

    #[test]
    fn today_is_judged_in_the_local_offset() {
        let plus_two = FixedOffset::east_opt(2 * 3600).unwrap();
        let now = at("2024-05-10T23:30:00Z");
        // 23:30 UTC is already the 11th at +2, so something from the evening
        // of the 10th (22:00 local) is yesterday, not today.
        assert_eq!(
            message_time(at("2024-05-10T20:00:00Z"), now, plus_two),
            "Yesterday at 22:00"
        );
    }

    #[test]
    fn day_label_spells_out_the_month() {
        assert_eq!(day_label("2026-11-05".parse().unwrap()), "November 5, 2026");
    }
}
