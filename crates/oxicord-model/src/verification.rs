//! The membership screening form a server asks new members to fill in, and
//! the profile shown beside it in the dialog.

use serde::Deserialize;
use serde_json::{Value, json};

use crate::Snowflake;

/// `GET /guilds/:id/profile`: what the card beside the form shows.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct GuildProfile {
    pub id: Snowflake,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub icon_hash: Option<String>,
    #[serde(default)]
    pub member_count: Option<u64>,
    #[serde(default)]
    pub online_count: Option<u64>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub custom_banner_hash: Option<String>,
    #[serde(default)]
    pub tag: Option<String>,
    /// The badge Discord picked for the server, 8 being discoverable.
    #[serde(default)]
    pub badge: Option<u64>,
    #[serde(default)]
    pub badge_hash: Option<String>,
    #[serde(default)]
    pub premium_tier: Option<u8>,
    #[serde(default)]
    pub premium_subscription_count: Option<u64>,
    /// The server's own accent colour, when it has one.
    #[serde(default)]
    pub brand_color_primary: Option<String>,
    #[serde(default)]
    pub features: Vec<String>,
}

/// `GET /guilds/:id/member-verification`.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct VerificationForm {
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub form_fields: Vec<FormField>,
    /// Some payloads carry a small guild snapshot with the form.
    #[serde(default)]
    pub guild: Option<Value>,
}

impl VerificationForm {
    /// True when all the fields are terms, the only kind the dialog can fill
    /// in so far. Other kinds need their own inputs.
    pub fn only_terms(&self) -> bool {
        self.form_fields.iter().all(|f| f.field_type == "TERMS")
    }

    /// True when at least one field is a written answer or a choice.
    pub fn has_manual_fields(&self) -> bool {
        self.form_fields.iter().any(|f| f.field_type != "TERMS")
    }

    /// The body `PUT /guilds/:id/requests/@me` expects. `answers` holds one
    /// response per field, in form order.
    pub fn submission(&self, answers: &[VerificationResponse]) -> Value {
        let fields: Vec<Value> = self
            .form_fields
            .iter()
            .enumerate()
            .map(|(index, field)| {
                let response = answers.get(index).map(VerificationResponse::value).unwrap_or(Value::Null);
                json!({
                    "field_type": field.field_type,
                    "label": field.label,
                    "description": field.description,
                    "required": field.required,
                    "values": field.values,
                    "choices": field.choices,
                    "response": response,
                })
            })
            .collect();
        json!({ "version": self.version, "form_fields": fields })
    }
}

/// What one form field was answered with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VerificationResponse {
    /// The terms checkbox.
    Terms(bool),
    /// A written answer.
    Text(String),
    /// The index of the chosen option.
    Choice(Option<usize>),
}

impl VerificationResponse {
    fn value(&self) -> Value {
        match self {
            Self::Terms(agreed) => json!(agreed),
            Self::Text(text) => json!(text),
            Self::Choice(choice) => match choice {
                Some(index) => json!(index),
                None => Value::Null,
            },
        }
    }
}

/// One question on the form.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct FormField {
    #[serde(default)]
    pub field_type: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub values: Vec<String>,
    /// Options for a multiple choice question.
    #[serde(default)]
    pub choices: Vec<String>,
}

/// The two answers the dialog opens with.
#[derive(Clone, Debug, Default)]
pub struct VerificationInfo {
    pub profile: GuildProfile,
    pub form: VerificationForm,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_live_form() {
        let form: VerificationForm = serde_json::from_str(
            r#"{"version":"2023-01-02T17:49:40.040000+00:00",
                "description":"Welcome",
                "form_fields":[{"field_type":"TERMS","label":"Read and agree to the server rules",
                "description":null,"automations":null,"required":true,
                "values":["Abide by the Discord ToS.","Have fun. :)"]}]}"#,
        )
        .unwrap();
        assert_eq!(form.form_fields.len(), 1);
        assert!(form.only_terms());
        assert_eq!(form.form_fields[0].values.len(), 2);
    }

    #[test]
    fn a_submission_answers_every_field() {
        let form: VerificationForm = serde_json::from_str(
            r#"{"version":"v1","form_fields":[
                {"field_type":"TERMS","label":"rules","required":true,"values":["one"]},
                {"field_type":"TEXT_INPUT","label":"why","required":false,"values":[]},
                {"field_type":"MULTIPLE_CHOICE","label":"pick","required":true,"choices":["a","b"]}]}"#,
        )
        .unwrap();
        let body = form.submission(&[
            VerificationResponse::Terms(true),
            VerificationResponse::Text("because".into()),
            VerificationResponse::Choice(Some(1)),
        ]);
        assert_eq!(body["version"], "v1");
        assert_eq!(body["form_fields"][0]["response"], true);
        assert_eq!(body["form_fields"][1]["response"], "because");
        assert_eq!(body["form_fields"][2]["response"], 1);
        assert_eq!(body["form_fields"][2]["choices"][1], "b");
        assert!(!form.only_terms());
        assert!(form.has_manual_fields());
    }

    #[test]
    fn reads_live_manual_fields() {
        let form: VerificationForm = serde_json::from_str(
            r#"{"version":"v1","form_fields":[
                {"field_type":"PARAGRAPH","label":"Why?","required":true,"values":[]},
                {"field_type":"TEXT_INPUT","label":"How?","required":true,"values":[]}]}"#,
        )
        .unwrap();
        assert!(form.has_manual_fields());
        assert_eq!(form.form_fields[0].label, "Why?");
    }

    #[test]
    fn reads_the_live_profile() {
        let profile: GuildProfile = serde_json::from_str(
            r#"{"id":"605462595239149568","name":"SCP | Roleplay","icon_hash":"a6ee",
                "member_count":182409,"online_count":42015,"description":"The server",
                "custom_banner_hash":"7854","tag":"SCP","badge":8,
                "badge_hash":"c8d0","premium_tier":3,"premium_subscription_count":209,
                "features":["COMMUNITY"]}"#,
        )
        .unwrap();
        assert_eq!(profile.name, "SCP | Roleplay");
        assert_eq!(profile.online_count, Some(42_015));
        assert_eq!(profile.tag.as_deref(), Some("SCP"));
        assert_eq!(profile.badge, Some(8));
        assert_eq!(profile.premium_subscription_count, Some(209));
    }
}
