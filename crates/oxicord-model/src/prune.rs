//! Pruning a server's inactive members: `GET /guilds/:id/prune?days=N` counts
//! who would go, `POST /guilds/:id/prune` removes them.

use serde::Deserialize;
use serde_json::{Value, json};

/// The day counts the prune dialog offers.
pub const PRUNE_DAYS: [u32; 3] = [1, 7, 30];

/// What both prune routes answer with. A write that skips the count answers
/// `null`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Deserialize)]
pub struct PruneResult {
    #[serde(default)]
    pub pruned: Option<u64>,
}

/// The body that actually prunes. The count is asked for so the page can say
/// how many members went.
pub fn prune_body(days: u32) -> Value {
    json!({ "days": days.clamp(1, 30), "compute_prune_count": true })
}

/// A prune preview or result as the page shows it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PruneState {
    pub days: u32,
    pub count: u64,
    /// True once the members were removed, false while it is only a preview.
    pub done: bool,
}

/// Sentence for a count of members, "1 member" or "3 members".
pub fn members_label(count: u64) -> String {
    if count == 1 { "1 member".to_owned() } else { format!("{count} members") }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_body_keeps_days_in_range_and_asks_for_the_count() {
        assert_eq!(prune_body(7), json!({ "days": 7, "compute_prune_count": true }));
        assert_eq!(prune_body(0)["days"], 1);
        assert_eq!(prune_body(90)["days"], 30);
    }

    #[test]
    fn the_answer_reads_with_or_without_a_count() {
        let got: PruneResult = serde_json::from_value(json!({ "pruned": 12 })).unwrap();
        assert_eq!(got.pruned, Some(12));
        let none: PruneResult = serde_json::from_value(json!({ "pruned": null })).unwrap();
        assert_eq!(none.pruned, None);
        let bare: PruneResult = serde_json::from_value(json!({})).unwrap();
        assert_eq!(bare.pruned, None);
    }

    #[test]
    fn counts_read_as_plain_words() {
        assert_eq!(members_label(1), "1 member");
        assert_eq!(members_label(0), "0 members");
        assert_eq!(members_label(5), "5 members");
    }
}
