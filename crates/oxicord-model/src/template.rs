//! Server templates, `GET /guilds/:id/templates`: a shareable snapshot of a
//! server's channels and roles that anyone can start a new server from.

use serde::Deserialize;
use serde_json::{Value, json};

/// Longest template name the API takes.
pub const MAX_TEMPLATE_NAME: usize = 100;
/// Longest template description the API takes.
pub const MAX_TEMPLATE_DESCRIPTION: usize = 120;

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct GuildTemplate {
    pub code: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub usage_count: u64,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
    /// The server changed since the template was last synced.
    #[serde(default)]
    pub is_dirty: Option<bool>,
}

impl GuildTemplate {
    /// The link people open to start a server from this template.
    pub fn url(&self) -> String {
        format!("https://discord.new/{}", self.code)
    }

    pub fn dirty(&self) -> bool {
        self.is_dirty.unwrap_or(false)
    }

    /// The day the template was last synced, `2026-11-11`.
    pub fn synced_on(&self) -> &str {
        let stamp = if self.updated_at.is_empty() { &self.created_at } else { &self.updated_at };
        stamp.get(..10).unwrap_or(stamp)
    }

    /// Whether the typed name or description differ from what is saved.
    pub fn changed(&self, name: &str, description: &str) -> bool {
        !name.trim().is_empty()
            && (name.trim() != self.name || description.trim() != self.description.as_deref().unwrap_or("").trim())
    }
}

/// The `POST` and `PATCH` body: a name and a description, which stays an empty
/// string rather than null when blank, as the web client sends it.
pub fn template_body(name: &str, description: &str) -> Value {
    json!({
        "name": name.trim().chars().take(MAX_TEMPLATE_NAME).collect::<String>(),
        "description": description.trim().chars().take(MAX_TEMPLATE_DESCRIPTION).collect::<String>(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> GuildTemplate {
        serde_json::from_value(json!({
            "code": "hgM48av5Q69A",
            "name": "Friends",
            "description": null,
            "usage_count": 3,
            "creator_id": "1",
            "created_at": "2026-11-01T10:00:00+00:00",
            "updated_at": "2026-11-09T18:30:00+00:00",
            "source_guild_id": "2",
            "is_dirty": true,
        }))
        .unwrap()
    }

    #[test]
    fn parses_what_the_api_sends() {
        let template = sample();
        assert_eq!(template.code, "hgM48av5Q69A");
        assert_eq!(template.usage_count, 3);
        assert!(template.dirty());
        assert_eq!(template.url(), "https://discord.new/hgM48av5Q69A");
        assert_eq!(template.synced_on(), "2026-11-09");
    }

    #[test]
    fn a_clean_template_is_not_dirty_and_falls_back_to_its_creation_day() {
        let template = GuildTemplate { created_at: "2026-11-01T10:00:00+00:00".into(), ..GuildTemplate::default() };
        assert!(!template.dirty());
        assert_eq!(template.synced_on(), "2026-11-01");
    }

    #[test]
    fn the_body_is_trimmed_and_capped() {
        let body = template_body("  Friends  ", "  ");
        assert_eq!(body["name"], "Friends");
        assert_eq!(body["description"], "");
        let long = "x".repeat(300);
        let body = template_body(&long, &long);
        assert_eq!(body["name"].as_str().unwrap().len(), MAX_TEMPLATE_NAME);
        assert_eq!(body["description"].as_str().unwrap().len(), MAX_TEMPLATE_DESCRIPTION);
    }

    #[test]
    fn only_a_real_edit_counts_as_changed() {
        let template = sample();
        assert!(!template.changed("Friends", ""));
        assert!(!template.changed(" Friends ", "  "));
        assert!(template.changed("Friends 2", ""));
        assert!(template.changed("Friends", "For friends"));
        assert!(!template.changed("   ", "x"), "a blank name can't be saved");
    }
}
