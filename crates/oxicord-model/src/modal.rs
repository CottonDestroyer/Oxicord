//! A bot's modal, delivered by `INTERACTION_MODAL_CREATE`: a title and the
//! text inputs to fill in.

use serde::Deserialize;

use crate::Snowflake;

/// The application that opened the modal. Only the id is needed to submit.
#[derive(Clone, Debug, Deserialize)]
pub struct ModalApplication {
    pub id: Snowflake,
}

#[derive(Clone, Debug, Deserialize)]
pub struct InteractionModal {
    pub id: Snowflake,
    #[serde(default)]
    pub channel_id: Option<Snowflake>,
    #[serde(default)]
    pub custom_id: String,
    #[serde(default)]
    pub application: Option<ModalApplication>,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub components: Vec<ModalRow>,
    #[serde(default)]
    pub nonce: Option<String>,
}

/// One action row of a modal. Only text inputs are kept.
#[derive(Clone, Debug, Deserialize)]
pub struct ModalRow {
    #[serde(default)]
    pub components: Vec<ModalInput>,
}

/// A text input: style 1 is a single line, style 2 a paragraph.
#[derive(Clone, Debug, Deserialize)]
pub struct ModalInput {
    #[serde(default, rename = "type")]
    pub kind: u8,
    pub custom_id: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub style: u8,
    #[serde(default)]
    pub min_length: Option<u32>,
    #[serde(default)]
    pub max_length: Option<u32>,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub value: Option<String>,
    #[serde(default)]
    pub placeholder: Option<String>,
}

impl ModalInput {
    pub fn paragraph(&self) -> bool {
        self.style == 2
    }
}

impl InteractionModal {
    /// The text inputs, in the order the modal draws them.
    pub fn inputs(&self) -> impl Iterator<Item = &ModalInput> {
        self.components
            .iter()
            .flat_map(|row| row.components.iter())
            .filter(|input| input.kind == 4)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_ticket_modal() {
        let text = include_str!("../../../fixtures/interaction-modal.json");
        let modal: InteractionModal = serde_json::from_str(text).unwrap();
        assert_eq!(modal.id, Snowflake(1_400_000_000_000_000_000));
        assert_eq!(modal.title, "Create a ticket");
        assert_eq!(modal.custom_id, "ticket-modal");
        assert_eq!(modal.application.as_ref().unwrap().id, Snowflake(123_456_789_012_345_678));
        assert_eq!(modal.nonce.as_deref(), Some("1400000000000000123"));

        let inputs: Vec<_> = modal.inputs().collect();
        assert_eq!(inputs.len(), 3);
        assert_eq!(inputs[0].custom_id, "subject");
        assert_eq!(inputs[0].label, "Subject");
        assert!(!inputs[0].paragraph());
        assert!(inputs[0].required);
        assert_eq!(inputs[0].min_length, Some(3));
        assert_eq!(inputs[0].max_length, Some(80));
        assert!(inputs[1].paragraph());
        assert_eq!(inputs[1].min_length, Some(10));
        assert!(!inputs[2].required);
        assert_eq!(inputs[2].max_length, Some(500));
    }
}
