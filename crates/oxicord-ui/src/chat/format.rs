//! Dates and times as the message list shows them.

use chrono::{DateTime, FixedOffset, NaiveDate, Utc};

/// "Today at 14:05", "Yesterday at 14:05", or the full date beyond that.
/// Clock format for the reader's 12 or 24 hour preference.
fn clock(hour12: bool) -> &'static str {
    if hour12 { "%-I:%M %p" } else { "%H:%M" }
}

pub fn message_time(at: DateTime<Utc>, now: DateTime<Utc>, offset: FixedOffset, hour12: bool) -> String {
    let local = at.with_timezone(&offset);
    let today = now.with_timezone(&offset).date_naive();
    let day = local.date_naive();
    let time = local.format(clock(hour12));

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

/// A `<t:unix:style>` timestamp as Discord renders it. The styles are t
/// (short time), T (long time), d (short date), D (long date), f (date and
/// time, the default), F (with the weekday) and R (relative).
pub fn timestamp(
    unix: i64,
    style: char,
    now: DateTime<Utc>,
    offset: FixedOffset,
    hour12: bool,
) -> String {
    let Some(at) = DateTime::from_timestamp(unix, 0) else {
        return "invalid date".to_owned();
    };
    let local = at.with_timezone(&offset);

    let pattern = match style {
        't' => clock(hour12).to_owned(),
        'T' => if hour12 { "%-I:%M:%S %p" } else { "%H:%M:%S" }.to_owned(),
        'd' => "%d/%m/%Y".to_owned(),
        'D' => "%B %-d, %Y".to_owned(),
        'F' => format!("%A, %B %-d, %Y {}", clock(hour12)),
        'R' => return relative(at.timestamp() - now.timestamp()),
        _ => format!("%B %-d, %Y {}", clock(hour12)),
    };
    local.format(&pattern).to_string()
}

/// "3 hours ago" or "in 2 days". `delta` is seconds from now, negative for
/// the past.
fn relative(delta: i64) -> String {
    let seconds = delta.unsigned_abs();
    let unit = |n: u64, name: &str| format!("{n} {name}{}", if n == 1 { "" } else { "s" });

    let span = match seconds {
        0..=59 => "a few seconds".to_owned(),
        60..=3599 => unit(seconds / 60, "minute"),
        3600..=86_399 => unit(seconds / 3600, "hour"),
        86_400..=2_591_999 => unit(seconds / 86_400, "day"),
        2_592_000..=31_535_999 => unit(seconds / 2_592_000, "month"),
        _ => unit(seconds / 31_536_000, "year"),
    };
    if delta < 0 { format!("{span} ago") } else { format!("in {span}") }
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
        assert_eq!(message_time(at("2024-05-10T09:05:00Z"), now, utc(), false), "Today at 09:05");
        assert_eq!(message_time(at("2024-05-09T23:59:00Z"), now, utc(), false), "Yesterday at 23:59");
        assert_eq!(message_time(at("2024-05-01T08:00:00Z"), now, utc(), false), "01/05/2024 08:00");
    }

    #[test]
    fn twelve_hour_clock_drops_the_leading_zero() {
        let now = at("2024-05-10T12:00:00Z");
        assert_eq!(message_time(at("2024-05-10T09:05:00Z"), now, utc(), true), "Today at 9:05 AM");
        assert_eq!(message_time(at("2024-05-10T21:30:00Z"), now, utc(), true), "Today at 9:30 PM");
        assert_eq!(timestamp(1_700_000_000, 't', now, utc(), true), "10:13 PM");
    }

    #[test]
    fn today_is_judged_in_the_local_offset() {
        let plus_two = FixedOffset::east_opt(2 * 3600).unwrap();
        let now = at("2024-05-10T23:30:00Z");
        // 23:30 UTC is already the 11th at +2, so something from the evening
        // of the 10th (22:00 local) is yesterday, not today.
        assert_eq!(
            message_time(at("2024-05-10T20:00:00Z"), now, plus_two, false),
            "Yesterday at 22:00"
        );
    }

    #[test]
    fn timestamp_styles() {
        // 2023-11-14 22:13:20 UTC
        let unix = 1_700_000_000;
        let now = at("2023-11-14T22:13:20Z");
        let t = |style| timestamp(unix, style, now, utc(), false);

        assert_eq!(t('t'), "22:13");
        assert_eq!(t('T'), "22:13:20");
        assert_eq!(t('d'), "14/11/2023");
        assert_eq!(t('D'), "November 14, 2023");
        assert_eq!(t('f'), "November 14, 2023 22:13");
        assert_eq!(t('F'), "Tuesday, November 14, 2023 22:13");
    }

    #[test]
    fn relative_timestamps_read_naturally() {
        let now = at("2023-11-14T22:13:20Z");
        let rel = |offset: i64| timestamp(1_700_000_000 + offset, 'R', now, utc(), false);

        assert_eq!(rel(-10), "a few seconds ago");
        assert_eq!(rel(-90), "1 minute ago");
        assert_eq!(rel(-3 * 3600), "3 hours ago");
        assert_eq!(rel(2 * 86_400), "in 2 days");
        assert_eq!(rel(-400 * 86_400), "1 year ago");
    }

    #[test]
    fn nonsense_timestamps_do_not_panic() {
        assert_eq!(timestamp(i64::MAX, 'f', at("2023-11-14T22:13:20Z"), utc(), false), "invalid date");
    }

    #[test]
    fn day_label_spells_out_the_month() {
        assert_eq!(day_label("2026-11-05".parse().unwrap()), "November 5, 2026");
    }
}
