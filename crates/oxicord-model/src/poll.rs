//! Polls: a question, its answers and the running tally. Parsed forgivingly
//! like components, a poll we cannot read is simply not shown.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer};
use serde_json::Value;

use crate::Emoji;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PollAnswer {
    pub id: u32,
    pub text: String,
    pub emoji: Option<Emoji>,
}

/// Votes for one answer, and whether we are among them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Tally {
    pub count: u32,
    pub me: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Poll {
    pub question: String,
    pub answers: Vec<PollAnswer>,
    pub expiry: Option<DateTime<Utc>>,
    pub multiselect: bool,
    /// Once true the vote is over and nothing changes.
    pub finalized: bool,
    pub tally: BTreeMap<u32, Tally>,
}

impl Poll {
    pub fn from_value(value: &Value) -> Option<Self> {
        let question = value.get("question")?.get("text")?.as_str()?.to_owned();
        let answers: Vec<PollAnswer> = value
            .get("answers")?
            .as_array()?
            .iter()
            .filter_map(|a| {
                let media = a.get("poll_media")?;
                Some(PollAnswer {
                    id: u32::try_from(a.get("answer_id")?.as_u64()?).ok()?,
                    text: media.get("text").and_then(Value::as_str).unwrap_or_default().to_owned(),
                    emoji: media.get("emoji").and_then(|e| serde_json::from_value(e.clone()).ok()),
                })
            })
            .collect();
        if answers.is_empty() {
            return None;
        }

        let results = value.get("results");
        let mut tally = BTreeMap::new();
        for entry in results
            .and_then(|r| r.get("answer_counts"))
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let id = entry.get("id").and_then(Value::as_u64).and_then(|i| u32::try_from(i).ok());
            if let Some(id) = id {
                tally.insert(
                    id,
                    Tally {
                        count: entry.get("count").and_then(Value::as_u64).unwrap_or(0) as u32,
                        me: entry.get("me_voted").and_then(Value::as_bool).unwrap_or(false),
                    },
                );
            }
        }

        Some(Self {
            question,
            answers,
            expiry: value.get("expiry").and_then(Value::as_str).and_then(|e| e.parse().ok()),
            multiselect: value.get("allow_multiselect").and_then(Value::as_bool).unwrap_or(false),
            finalized: results
                .and_then(|r| r.get("is_finalized"))
                .and_then(Value::as_bool)
                .unwrap_or(false),
            tally,
        })
    }

    pub fn votes(&self, answer: u32) -> Tally {
        self.tally.get(&answer).copied().unwrap_or_default()
    }

    pub fn total_votes(&self) -> u32 {
        self.tally.values().map(|t| t.count).sum()
    }

    /// The answers we voted for.
    pub fn my_votes(&self) -> Vec<u32> {
        self.tally.iter().filter(|(_, t)| t.me).map(|(id, _)| *id).collect()
    }

    /// The answers after we pick `answer`: replaces our vote on a single
    /// choice poll, toggles it on a multiple choice one.
    pub fn votes_after_picking(&self, answer: u32) -> Vec<u32> {
        let mut mine = self.my_votes();
        if self.multiselect {
            match mine.iter().position(|a| *a == answer) {
                Some(i) => {
                    mine.remove(i);
                }
                None => mine.push(answer),
            }
            mine
        } else if mine == [answer] {
            Vec::new()
        } else {
            vec![answer]
        }
    }

    /// Records our votes at once, ahead of the server's tally.
    pub fn set_my_votes(&mut self, votes: &[u32]) {
        for answer in &self.answers {
            let entry = self.tally.entry(answer.id).or_default();
            let wanted = votes.contains(&answer.id);
            if wanted && !entry.me {
                entry.count += 1;
            } else if !wanted && entry.me {
                entry.count = entry.count.saturating_sub(1);
            }
            entry.me = wanted;
        }
    }

    /// Applies someone's vote or retraction from the gateway.
    pub fn apply_vote(&mut self, answer: u32, add: bool, mine: bool) {
        let entry = self.tally.entry(answer).or_default();
        if add {
            entry.count += 1;
        } else {
            entry.count = entry.count.saturating_sub(1);
        }
        if mine {
            entry.me = add;
        }
    }
}

/// A message's poll, or none when it has no poll or we cannot read it.
pub fn poll<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<Poll>, D::Error> {
    Ok(Option::<Value>::deserialize(deserializer)?.and_then(|v| Poll::from_value(&v)))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn sample(multi: bool) -> Poll {
        Poll::from_value(&json!({
            "question": { "text": "Pizza?" },
            "answers": [
                { "answer_id": 1, "poll_media": { "text": "Yes" } },
                { "answer_id": 2, "poll_media": { "text": "No", "emoji": { "name": "x" } } }
            ],
            "expiry": "2030-01-01T00:00:00+00:00",
            "allow_multiselect": multi,
            "results": { "is_finalized": false,
                         "answer_counts": [{ "id": 1, "count": 3, "me_voted": true }, { "id": 2, "count": 1 }] }
        }))
        .unwrap()
    }

    #[test]
    fn reads_question_answers_and_tally() {
        let poll = sample(false);
        assert_eq!(poll.question, "Pizza?");
        assert_eq!(poll.answers.len(), 2);
        assert_eq!(poll.total_votes(), 4);
        assert_eq!(poll.my_votes(), vec![1]);
        assert!(poll.answers[1].emoji.is_some());
        assert!(poll.expiry.is_some());
    }

    #[test]
    fn a_poll_without_answers_or_a_question_is_not_shown() {
        assert!(Poll::from_value(&json!({ "question": { "text": "x" }, "answers": [] })).is_none());
        assert!(Poll::from_value(&json!({ "answers": [] })).is_none());
        assert!(Poll::from_value(&json!("nope")).is_none());
    }

    #[test]
    fn single_choice_replaces_and_a_repeat_clears() {
        let poll = sample(false);
        assert_eq!(poll.votes_after_picking(2), vec![2]);
        assert!(poll.votes_after_picking(1).is_empty());
    }

    #[test]
    fn multiple_choice_toggles() {
        let poll = sample(true);
        assert_eq!(poll.votes_after_picking(2), vec![1, 2]);
        assert!(poll.votes_after_picking(1).is_empty());
    }

    #[test]
    fn optimistic_votes_move_the_counts() {
        let mut poll = sample(false);
        poll.set_my_votes(&[2]);
        assert_eq!(poll.votes(1), Tally { count: 2, me: false });
        assert_eq!(poll.votes(2), Tally { count: 2, me: true });
        poll.apply_vote(1, true, false);
        assert_eq!(poll.votes(1).count, 3);
        poll.apply_vote(2, false, true);
        assert_eq!(poll.votes(2), Tally { count: 1, me: false });
    }
}
