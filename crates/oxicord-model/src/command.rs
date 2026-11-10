//! Slash commands as bots register them. Parsing is forgiving, a command or
//! option we cannot read is dropped instead of failing the whole list.

use serde::Deserialize;
use serde_json::{Value, json};

use crate::Snowflake;

/// Discord's option kinds.
pub mod kind {
    pub const SUB_COMMAND: u8 = 1;
    pub const SUB_COMMAND_GROUP: u8 = 2;
    pub const STRING: u8 = 3;
    pub const INTEGER: u8 = 4;
    pub const BOOLEAN: u8 = 5;
    pub const USER: u8 = 6;
    pub const CHANNEL: u8 = 7;
    pub const ROLE: u8 = 8;
    pub const MENTIONABLE: u8 = 9;
    pub const NUMBER: u8 = 10;
    pub const ATTACHMENT: u8 = 11;
}

#[derive(Clone, Debug, PartialEq)]
pub struct Choice {
    pub name: String,
    pub value: Value,
}

/// One row of a bot's live autocomplete answer.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct AutocompleteChoice {
    #[serde(default)]
    pub name: String,
    /// The name in the reader's language, when the bot sent one.
    #[serde(default)]
    pub name_localized: Option<String>,
    #[serde(default)]
    pub value: Value,
}

impl AutocompleteChoice {
    pub fn display_name(&self) -> &str {
        self.name_localized.as_deref().unwrap_or(&self.name)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CommandOption {
    pub kind: u8,
    pub name: String,
    pub description: String,
    pub required: bool,
    pub choices: Vec<Choice>,
    /// Sub commands and groups hold more options.
    pub options: Vec<CommandOption>,
    /// The bot fills choices in live, which this client does not support.
    pub autocomplete: bool,
}

impl CommandOption {
    fn from_value(value: &Value) -> Option<Self> {
        let text = |key: &str| value.get(key).and_then(Value::as_str).map(str::to_owned);
        Some(Self {
            kind: u8::try_from(value.get("type")?.as_u64()?).ok()?,
            name: text("name")?,
            description: text("description").unwrap_or_default(),
            required: value.get("required").and_then(Value::as_bool).unwrap_or(false),
            choices: value
                .get("choices")
                .and_then(Value::as_array)
                .map(|list| {
                    list.iter()
                        .filter_map(|c| {
                            Some(Choice { name: c.get("name")?.as_str()?.to_owned(), value: c.get("value")?.clone() })
                        })
                        .collect()
                })
                .unwrap_or_default(),
            options: options_of(value),
            autocomplete: value.get("autocomplete").and_then(Value::as_bool).unwrap_or(false),
        })
    }
}

fn options_of(value: &Value) -> Vec<CommandOption> {
    value
        .get("options")
        .and_then(Value::as_array)
        .map(|list| list.iter().filter_map(CommandOption::from_value).collect())
        .unwrap_or_default()
}

/// Every command usable somewhere, with the apps that own them.
#[derive(Clone, Debug, Default)]
pub struct CommandIndex {
    pub commands: Vec<SlashCommand>,
    pub apps: Vec<(Snowflake, String)>,
}

/// A chat input command as it appears in the picker.
#[derive(Clone, Debug, PartialEq)]
pub struct SlashCommand {
    pub id: Snowflake,
    pub application_id: Snowflake,
    /// What the user types, such as `ban` or `settings notifications`.
    pub name: String,
    pub description: String,
    pub version: String,
    /// The options of the command or sub command to run.
    pub options: Vec<CommandOption>,
    /// The sub command names leading to `options`, outermost first, with
    /// their option kind.
    pub path: Vec<(String, u8)>,
    /// The registered command the bot knows, sent back with a run.
    pub root: Value,
    /// The top level name, which the interaction names as the command.
    pub root_name: String,
}

impl SlashCommand {
    /// Reads a registered command and flattens its sub commands into
    /// separate runnable entries. Anything that is not a chat command is
    /// ignored.
    pub fn flatten(value: &Value) -> Vec<Self> {
        let id = value.get("id").and_then(Value::as_str).and_then(|s| s.parse().ok()).map(Snowflake);
        let app = value
            .get("application_id")
            .and_then(Value::as_str)
            .and_then(|s| s.parse().ok())
            .map(Snowflake);
        let (Some(id), Some(application_id)) = (id, app) else { return Vec::new() };
        // Type 1 is a chat input command, the others live in context menus.
        if value.get("type").and_then(Value::as_u64).unwrap_or(1) != 1 {
            return Vec::new();
        }
        let Some(root_name) = value.get("name").and_then(Value::as_str) else { return Vec::new() };
        let base = Self {
            id,
            application_id,
            name: root_name.to_owned(),
            description: value.get("description").and_then(Value::as_str).unwrap_or_default().to_owned(),
            version: value.get("version").and_then(Value::as_str).unwrap_or_default().to_owned(),
            options: options_of(value),
            path: Vec::new(),
            root: value.clone(),
            root_name: root_name.to_owned(),
        };

        let mut out = Vec::new();
        collect(&base, &mut out);
        out
    }

    /// The `options` array of an interaction for these answers: one value per
    /// option name, wrapped in the sub commands that lead here.
    pub fn interaction_options(&self, answers: &[(String, u8, Value)]) -> Value {
        let mut options: Value = answers
            .iter()
            .map(|(name, kind, value)| json!({ "type": kind, "name": name, "value": value }))
            .collect::<Vec<_>>()
            .into();
        for (name, kind) in self.path.iter().rev() {
            options = json!([{ "type": kind, "name": name, "options": options }]);
        }
        options
    }

    /// The `options` array of an autocomplete interaction: the same answers,
    /// with the option being typed marked `focused`.
    pub fn autocomplete_options(&self, answers: &[(String, u8, Value)], focused: &str) -> Value {
        let mut options: Value = answers
            .iter()
            .map(|(name, kind, value)| {
                let mut option = json!({ "type": kind, "name": name, "value": value });
                if name == focused {
                    option["focused"] = json!(true);
                }
                option
            })
            .collect::<Vec<_>>()
            .into();
        for (name, kind) in self.path.iter().rev() {
            options = json!([{ "type": kind, "name": name, "options": options }]);
        }
        options
    }
}

/// Splits sub commands out: a command with sub commands is only a folder.
fn collect(command: &SlashCommand, out: &mut Vec<SlashCommand>) {
    let has_children = command
        .options
        .iter()
        .any(|o| o.kind == kind::SUB_COMMAND || o.kind == kind::SUB_COMMAND_GROUP);
    if !has_children {
        out.push(command.clone());
        return;
    }
    for option in &command.options {
        if option.kind != kind::SUB_COMMAND && option.kind != kind::SUB_COMMAND_GROUP {
            continue;
        }
        let mut child = command.clone();
        child.name = format!("{} {}", command.name, option.name);
        child.description = option.description.clone();
        child.options = option.options.clone();
        child.path.push((option.name.clone(), option.kind));
        collect(&child, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registered() -> Value {
        json!({
            "id": "10", "application_id": "20", "name": "settings", "description": "Server settings",
            "version": "99", "type": 1,
            "options": [
                { "type": 1, "name": "view", "description": "Show them" },
                { "type": 2, "name": "log", "description": "Logging", "options": [
                    { "type": 1, "name": "channel", "description": "Pick one", "options": [
                        { "type": 7, "name": "where", "description": "Which", "required": true } ] } ] }
            ]
        })
    }

    #[test]
    fn sub_commands_become_their_own_entries() {
        let flat = SlashCommand::flatten(&registered());
        let names: Vec<_> = flat.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["settings view", "settings log channel"]);
        assert_eq!(flat[1].options.len(), 1);
        assert!(flat[1].options[0].required);
        assert_eq!(flat[1].root_name, "settings");
    }

    #[test]
    fn a_plain_command_stays_whole() {
        let flat = SlashCommand::flatten(&json!({
            "id": "1", "application_id": "2", "name": "ping", "description": "Pong",
            "options": [{ "type": 3, "name": "text", "choices": [{ "name": "A", "value": "a" }] }]
        }));
        assert_eq!(flat.len(), 1);
        assert_eq!(flat[0].options[0].choices[0].value, json!("a"));
    }

    #[test]
    fn context_menu_commands_and_junk_are_ignored() {
        assert!(SlashCommand::flatten(&json!({ "id": "1", "application_id": "2", "name": "x", "type": 2 })).is_empty());
        assert!(SlashCommand::flatten(&json!({ "name": "no ids" })).is_empty());
        assert!(SlashCommand::flatten(&json!("nope")).is_empty());
    }

    #[test]
    fn answers_are_wrapped_in_the_sub_command_path() {
        let flat = SlashCommand::flatten(&registered());
        let options = flat[1].interaction_options(&[("where".into(), 7, json!("55"))]);
        assert_eq!(
            options,
            json!([{ "type": 2, "name": "log", "options": [
                { "type": 1, "name": "channel", "options": [{ "type": 7, "name": "where", "value": "55" }] } ] }])
        );
    }

    #[test]
    fn an_autocomplete_names_the_focused_option() {
        let flat = SlashCommand::flatten(&json!({
            "id": "1", "application_id": "2", "name": "help", "description": "Help",
            "options": [{ "type": 3, "name": "command", "autocomplete": true }]
        }));
        let options = flat[0].autocomplete_options(
            &[("command".into(), 3, json!("rai"))],
            "command",
        );
        assert_eq!(
            options,
            json!([{ "type": 3, "name": "command", "value": "rai", "focused": true }])
        );
    }

    #[test]
    fn autocomplete_choices_prefer_the_localized_name() {
        let choice: AutocompleteChoice =
            serde_json::from_value(json!({ "name": "raid", "name_localized": "raid (fr)", "value": "raid" }))
                .unwrap();
        assert_eq!(choice.display_name(), "raid (fr)");
        let plain: AutocompleteChoice =
            serde_json::from_value(json!({ "name": "afk", "value": "afk" })).unwrap();
        assert_eq!(plain.display_name(), "afk");
    }
}
