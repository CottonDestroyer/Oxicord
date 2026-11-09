//! Server onboarding, `GET /guilds/:id/onboarding`: the questions shown to a
//! newcomer and the channels and roles each answer grants.

use std::collections::{BTreeMap, HashMap, HashSet};

use serde::{Deserialize, Serialize};
use serde_repr::Deserialize_repr;

use crate::{Emoji, Snowflake};

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

#[derive(Clone, Debug, Default, Deserialize)]
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
#[derive(Clone, Debug, Deserialize)]
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
#[derive(Clone, Debug, Deserialize)]
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
#[derive(Clone, Debug, Deserialize)]
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

impl Onboarding {
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
