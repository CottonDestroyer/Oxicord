use oxicord_model::Snowflake;
use reqwest::Method;
use serde_json::{Value, json};

use crate::{ApiClient, ApiError};

/// A click on a message's button or menu.
#[derive(Clone, Debug)]
pub struct ComponentClick {
    pub guild: Option<Snowflake>,
    pub channel: Snowflake,
    pub message: Snowflake,
    pub message_flags: u64,
    pub application: Snowflake,
    pub session_id: String,
    pub component_type: u8,
    pub custom_id: String,
    /// The chosen options of a menu, empty for buttons.
    pub values: Vec<String>,
    pub nonce: String,
}

/// The answers to a bot's modal.
#[derive(Clone, Debug)]
pub struct ModalSubmit {
    pub guild: Option<Snowflake>,
    pub channel: Snowflake,
    pub application: Snowflake,
    pub session_id: String,
    pub custom_id: String,
    /// Input custom id and what was typed in it, in the order the modal drew
    /// the inputs.
    pub answers: Vec<(String, String)>,
    pub nonce: String,
}

/// The request body for a modal submit, built apart from the network so it
/// can be checked on its own.
pub fn modal_body(submit: &ModalSubmit) -> Value {
    let components: Vec<Value> = submit
        .answers
        .iter()
        .map(|(custom_id, value)| {
            json!({
                // Action row holding one text input.
                "type": 1,
                "components": [{
                    "type": 4,
                    "custom_id": custom_id,
                    "value": value,
                }],
            })
        })
        .collect();
    let mut body = json!({
        // Interaction type 5 is a modal submit.
        "type": 5,
        "nonce": submit.nonce,
        "channel_id": submit.channel.to_string(),
        "application_id": submit.application.to_string(),
        "session_id": submit.session_id,
        "data": {
            "custom_id": submit.custom_id,
            "components": components,
        },
    });
    if let Some(guild) = submit.guild {
        body["guild_id"] = json!(guild.to_string());
    }
    body
}

/// The request body for a click, built apart from the network so it can be
/// checked on its own.
pub fn click_body(click: &ComponentClick) -> Value {
    let mut data = json!({
        "component_type": click.component_type,
        "custom_id": click.custom_id,
    });
    if click.component_type != 2 {
        data["type"] = json!(click.component_type);
        data["values"] = json!(click.values);
    }
    let mut body = json!({
        // Interaction type 3 is a message component.
        "type": 3,
        "nonce": click.nonce,
        "channel_id": click.channel.to_string(),
        "message_flags": click.message_flags,
        "message_id": click.message.to_string(),
        "application_id": click.application.to_string(),
        "session_id": click.session_id,
        "data": data,
    });
    if let Some(guild) = click.guild {
        body["guild_id"] = json!(guild.to_string());
    }
    body
}

impl ApiClient {
    /// Sets our votes on a poll to exactly `answers`. An empty list removes
    /// them.
    pub async fn vote_poll(
        &self,
        channel: Snowflake,
        message: Snowflake,
        answers: &[u32],
    ) -> Result<(), ApiError> {
        let body = json!({ "answer_ids": answers.iter().map(u32::to_string).collect::<Vec<_>>() });
        self.empty(
            Method::PUT,
            &format!("/channels/{channel}/polls/{message}/answers/@me"),
            &format!("PUT /channels/{channel}/polls/:id/answers/@me"),
            Some(&body),
        )
        .await
    }

    /// Presses a button or picks from a menu. The app answers through the
    /// gateway, not in this response.
    pub async fn click_component(&self, click: &ComponentClick) -> Result<(), ApiError> {
        self.empty(
            Method::POST,
            "/interactions",
            "POST /interactions",
            Some(&click_body(click)),
        )
        .await
    }

    /// Sends the answers to a bot modal. The app answers through the gateway,
    /// not in this response.
    pub async fn submit_modal(&self, submit: &ModalSubmit) -> Result<(), ApiError> {
        self.empty(
            Method::POST,
            "/interactions",
            "POST /interactions",
            Some(&modal_body(submit)),
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn click(component_type: u8, values: Vec<String>) -> ComponentClick {
        ComponentClick {
            guild: Some(Snowflake(9)),
            channel: Snowflake(5),
            message: Snowflake(7),
            message_flags: 0,
            application: Snowflake(3),
            session_id: "sess".into(),
            component_type,
            custom_id: "id".into(),
            values,
            nonce: "123".into(),
        }
    }

    #[test]
    fn a_button_click_names_the_message_and_app() {
        let body = click_body(&click(2, vec![]));
        assert_eq!(body["type"], 3);
        assert_eq!(body["message_id"], "7");
        assert_eq!(body["application_id"], "3");
        assert_eq!(body["guild_id"], "9");
        assert_eq!(body["data"]["custom_id"], "id");
        assert!(body["data"].get("values").is_none());
    }

    #[test]
    fn a_menu_pick_carries_its_values() {
        let body = click_body(&click(3, vec!["a".into()]));
        assert_eq!(body["data"]["values"], json!(["a"]));
        assert_eq!(body["data"]["component_type"], 3);
    }

    #[test]
    fn direct_messages_leave_the_guild_out() {
        let mut c = click(2, vec![]);
        c.guild = None;
        assert!(click_body(&c).get("guild_id").is_none());
    }

    #[test]
    fn a_modal_submit_names_its_fields() {
        let body = modal_body(&ModalSubmit {
            guild: Some(Snowflake(9)),
            channel: Snowflake(5),
            application: Snowflake(3),
            session_id: "sess".into(),
            custom_id: "ticket-modal".into(),
            answers: vec![
                ("subject".into(), "Hello".into()),
                ("details".into(), "Need help".into()),
            ],
            nonce: "123".into(),
        });
        assert_eq!(body["type"], 5);
        assert_eq!(body["nonce"], "123");
        assert_eq!(body["channel_id"], "5");
        assert_eq!(body["application_id"], "3");
        assert_eq!(body["guild_id"], "9");
        assert_eq!(body["data"]["custom_id"], "ticket-modal");
        assert_eq!(body["data"]["components"][0]["type"], 1);
        assert_eq!(body["data"]["components"][0]["components"][0]["type"], 4);
        assert_eq!(body["data"]["components"][0]["components"][0]["custom_id"], "subject");
        assert_eq!(body["data"]["components"][0]["components"][0]["value"], "Hello");
        assert_eq!(body["data"]["components"][1]["components"][0]["custom_id"], "details");
    }
}
