//! Server onboarding, `GET /guilds/:id/onboarding`: the questions shown to a
//! newcomer and the channels and roles each answer grants.

use std::collections::{BTreeMap, HashMap, HashSet};

use serde::{Deserialize, Serialize};
use serde_repr::Deserialize_repr;

use crate::{Emoji, Snowflake};

/// Most questions a server can ask in onboarding.
pub const MAX_ONBOARDING_PROMPTS: usize = 5;
/// Most answers one question can offer.
pub const MAX_ONBOARDING_OPTIONS: usize = 50;

/// How onboarding is presented.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize_repr)]
#[repr(u8)]
pub enum OnboardingMode {
    #[default]
    Default = 0,
    Advanced = 1,
    #[serde(other)]
    Unknown = 255,
}

/// Whether a prompt's options are checkboxes or a drop-down.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize_repr)]
#[repr(u8)]
pub enum PromptType {
    #[default]
    MultipleChoice = 0,
    Dropdown = 1,
    #[serde(other)]
    Unknown = 255,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct Onboarding {
    pub guild_id: Snowflake,
    #[serde(default)]
    pub prompts: Vec<OnboardingPrompt>,
    #[serde(default)]
    pub default_channel_ids: Vec<Snowflake>,
    #[serde(default)]
    pub responses: Vec<OnboardingResponse>,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub mode: OnboardingMode,
    #[serde(default)]
    pub below_requirements: bool,
}

/// One question, with the answers it offers.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct OnboardingPrompt {
    pub id: Snowflake,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub options: Vec<OnboardingOption>,
    #[serde(default)]
    pub single_select: bool,
    #[serde(default)]
    pub disabled: bool,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub in_onboarding: bool,
    #[serde(default, rename = "type")]
    pub kind: PromptType,
}

/// One answer: a title, a description and the channels and roles it opens.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct OnboardingOption {
    pub id: Snowflake,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub emoji: Option<Emoji>,
    #[serde(default)]
    pub role_ids: Vec<Snowflake>,
    #[serde(default)]
    pub channel_ids: Vec<Snowflake>,
}

/// What we picked for a prompt.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct OnboardingResponse {
    pub prompt_id: Snowflake,
    #[serde(default)]
    pub option_ids: Vec<Snowflake>,
}

/// The body of `PUT /guilds/:id/onboarding-responses`: every answered option,
/// plus the time each prompt and option was last shown.
#[derive(Clone, Debug, Default, Serialize)]
pub struct OnboardingSubmission {
    pub onboarding_responses: Vec<Snowflake>,
    pub onboarding_prompts_seen: BTreeMap<Snowflake, u64>,
    pub onboarding_responses_seen: BTreeMap<Snowflake, u64>,
}

/// What the endpoint answers with: the stored option ids and seen maps.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct OnboardingUpdate {
    #[serde(default)]
    pub onboarding_responses: Vec<Snowflake>,
    #[serde(default)]
    pub onboarding_prompts_seen: BTreeMap<Snowflake, u64>,
    #[serde(default)]
    pub onboarding_responses_seen: BTreeMap<Snowflake, u64>,
}

impl OnboardingPrompt {
    /// Offers another answer, unless the question is full. `id` only has to be
    /// unique among this question's options: the server assigns the real one.
    pub fn add_option(&mut self, id: u64) -> bool {
        if self.options.len() >= MAX_ONBOARDING_OPTIONS {
            return false;
        }
        self.options.push(OnboardingOption {
            id: Snowflake(id),
            title: String::new(),
            description: String::new(),
            emoji: None,
            role_ids: Vec::new(),
            channel_ids: Vec::new(),
        });
        true
    }
}

impl Onboarding {
    /// Asks another question, unless the limit is reached. `id` is a
    /// throwaway, as the web client uses the time in milliseconds.
    pub fn add_prompt(&mut self, id: u64) -> bool {
        if self.prompts.iter().filter(|p| p.in_onboarding).count() >= MAX_ONBOARDING_PROMPTS {
            return false;
        }
        self.prompts.push(OnboardingPrompt {
            id: Snowflake(id),
            title: String::new(),
            options: Vec::new(),
            single_select: false,
            disabled: false,
            required: false,
            in_onboarding: true,
            kind: PromptType::MultipleChoice,
        });
        true
    }

    /// Adds a channel everyone lands in, or removes it when it is there.
    pub fn toggle_default_channel(&mut self, channel: Snowflake) {
        match self.default_channel_ids.iter().position(|c| *c == channel) {
            Some(at) => {
                self.default_channel_ids.remove(at);
            }
            None => self.default_channel_ids.push(channel),
        }
    }

    /// The `PUT /guilds/:id/onboarding` body, shaped as the web client sends
    /// it. A question or answer with no title is left out.
    pub fn edit_body(&self) -> serde_json::Value {
        let ids = |ids: &[Snowflake]| ids.iter().map(ToString::to_string).collect::<Vec<_>>();
        let prompts = self
            .prompts
            .iter()
            .filter(|prompt| !prompt.title.trim().is_empty())
            .map(|prompt| {
                let options = prompt
                    .options
                    .iter()
                    .filter(|option| !option.title.trim().is_empty())
                    .map(|option| {
                        let emoji = option.emoji.as_ref();
                        serde_json::json!({
                            "id": option.id.to_string(),
                            "title": option.title.trim(),
                            "description": option.description.trim(),
                            "channel_ids": ids(&option.channel_ids),
                            "role_ids": ids(&option.role_ids),
                            "emoji_id": emoji.and_then(|e| e.id).map(|e| e.to_string()),
                            "emoji_name": emoji.and_then(|e| e.name.clone()),
                            "emoji_animated": emoji.is_some_and(|e| e.animated),
                        })
                    })
                    .collect::<Vec<_>>();
                serde_json::json!({
                    "id": prompt.id.to_string(),
                    "title": prompt.title.trim(),
                    "options": options,
                    "single_select": prompt.single_select,
                    "disabled": prompt.disabled,
                    "required": prompt.required,
                    "in_onboarding": prompt.in_onboarding,
                    "type": if prompt.kind == PromptType::Dropdown { 1 } else { 0 },
                })
            })
            .collect::<Vec<_>>();
        serde_json::json!({
            "enabled": self.enabled,
            "mode": if self.mode == OnboardingMode::Advanced { 1 } else { 0 },
            "default_channel_ids": ids(&self.default_channel_ids),
            "prompts": prompts,
        })
    }

    /// Builds the update from the prompts that are part of onboarding and the
    /// options picked for them. `now` is the time the guide was shown, in
    /// milliseconds, as the client sends it.
    pub fn submission(
        &self,
        selected: &HashMap<Snowflake, HashSet<Snowflake>>,
        now: u64,
    ) -> OnboardingSubmission {
        let mut submission = OnboardingSubmission::default();
        for prompt in self.prompts.iter().filter(|prompt| prompt.in_onboarding) {
            submission.onboarding_prompts_seen.insert(prompt.id, now);
            let picked = selected.get(&prompt.id);
            for option in &prompt.options {
                submission.onboarding_responses_seen.insert(option.id, now);
                if picked.is_some_and(|picked| picked.contains(&option.id)) {
                    submission.onboarding_responses.push(option.id);
                }
            }
        }
        submission
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_scp_onboarding() {
        let text = include_str!("../../../fixtures/onboarding.json");
        let onboarding: Onboarding = serde_json::from_str(text).unwrap();
        assert_eq!(onboarding.guild_id, Snowflake(605_462_595_239_149_568));
        assert!(onboarding.enabled);
        assert_eq!(onboarding.mode, OnboardingMode::Default);
        assert!(!onboarding.below_requirements);
        assert_eq!(onboarding.default_channel_ids.len(), 19);
        assert!(onboarding.responses.is_empty());
        assert_eq!(onboarding.prompts.len(), 1);
        let prompt = &onboarding.prompts[0];
        assert_eq!(prompt.id, Snowflake(1_496_185_137_216_356_375));
        assert_eq!(prompt.title, "Do you want to be pinged for new update teasers?");
        assert_eq!(prompt.kind, PromptType::MultipleChoice);
        assert!(prompt.single_select);
        assert!(!prompt.required);
        assert!(!prompt.in_onboarding);
        assert_eq!(prompt.options.len(), 1);
        let option = &prompt.options[0];
        assert_eq!(option.id, Snowflake(1_496_185_137_216_356_378));
        assert_eq!(option.title, "Yes!");
        assert!(option.description.is_empty());
        assert_eq!(option.role_ids, vec![Snowflake(1_496_183_796_121_342_212)]);
        assert!(option.channel_ids.is_empty());
        let emoji = option.emoji.as_ref().unwrap();
        assert_eq!(emoji.id, Some(Snowflake(609_164_124_831_875_114)));
        assert_eq!(emoji.name.as_deref(), Some("Yes"));
        assert!(!emoji.animated);
    }

    #[test]
    fn the_edit_body_matches_what_the_web_client_sends() {
        let text = include_str!("../../../fixtures/onboarding.json");
        let onboarding: Onboarding = serde_json::from_str(text).unwrap();
        let body = onboarding.edit_body();
        assert_eq!(body["enabled"], true);
        assert_eq!(body["mode"], 0);
        assert_eq!(body["default_channel_ids"].as_array().unwrap().len(), 19);
        let prompt = &body["prompts"][0];
        assert_eq!(prompt["id"], "1496185137216356375");
        assert_eq!(prompt["single_select"], true);
        assert_eq!(prompt["type"], 0);
        let option = &prompt["options"][0];
        assert_eq!(option["title"], "Yes!");
        assert_eq!(option["role_ids"][0], "1496183796121342212");
        assert_eq!(option["emoji_id"], "609164124831875114");
        assert_eq!(option["emoji_name"], "Yes");
        assert_eq!(option["emoji_animated"], false);
    }

    #[test]
    fn untitled_questions_and_answers_are_not_sent() {
        let mut onboarding = Onboarding::default();
        assert!(onboarding.add_prompt(1));
        assert!(onboarding.add_prompt(2));
        onboarding.prompts[0].title = "  Pick one  ".into();
        assert!(onboarding.prompts[0].add_option(10));
        assert!(onboarding.prompts[0].add_option(11));
        onboarding.prompts[0].options[0].title = " Yes ".into();
        let body = onboarding.edit_body();
        assert_eq!(body["prompts"].as_array().unwrap().len(), 1, "the blank question is dropped");
        assert_eq!(body["prompts"][0]["title"], "Pick one");
        assert_eq!(body["prompts"][0]["options"].as_array().unwrap().len(), 1, "so is the blank answer");
        assert_eq!(body["prompts"][0]["options"][0]["title"], "Yes");
    }

    #[test]
    fn questions_are_capped_and_default_channels_toggle() {
        let mut onboarding = Onboarding::default();
        for id in 0..MAX_ONBOARDING_PROMPTS as u64 {
            assert!(onboarding.add_prompt(id));
        }
        assert!(!onboarding.add_prompt(99), "the sixth question is refused");

        onboarding.toggle_default_channel(Snowflake(4));
        onboarding.toggle_default_channel(Snowflake(5));
        onboarding.toggle_default_channel(Snowflake(4));
        assert_eq!(onboarding.default_channel_ids, vec![Snowflake(5)]);
    }

    #[test]
    fn builds_the_update_body() {
        let text = include_str!("../../../fixtures/onboarding.json");
        let mut onboarding: Onboarding = serde_json::from_str(text).unwrap();
        onboarding.prompts[0].in_onboarding = true;
        let prompt = &onboarding.prompts[0];
        let (prompt_id, option_id) = (prompt.id, prompt.options[0].id);
        let mut selected = HashMap::new();
        selected.insert(prompt_id, HashSet::from([option_id]));

        let now = 1_700_000_000_000;
        let body = onboarding.submission(&selected, now);
        assert_eq!(body.onboarding_responses, vec![option_id]);
        assert_eq!(body.onboarding_prompts_seen.get(&prompt_id), Some(&now));
        assert_eq!(body.onboarding_responses_seen.get(&option_id), Some(&now));

        let json = serde_json::to_value(&body).unwrap();
        assert_eq!(json["onboarding_responses"][0], option_id.to_string());
        assert_eq!(json["onboarding_prompts_seen"][prompt_id.to_string()], now);
        assert_eq!(json["onboarding_responses_seen"][option_id.to_string()], now);
    }
}
