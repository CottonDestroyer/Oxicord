use oxicord_model::{SlashCommand, Snowflake};
use reqwest::Method;
use serde_json::{Value, json};

use crate::{ApiClient, ApiError};

/// What the command index said.
#[derive(Debug)]
pub enum IndexOutcome {
    Ready {
        commands: Vec<SlashCommand>,
        /// The apps the commands belong to, for showing who owns what.
        apps: Vec<(Snowflake, String)>,
    },
    /// Discord is still building the list and wants another request later.
    Pending,
}

/// A command ready to run, with the answers to its options.
#[derive(Clone, Debug)]
pub struct CommandRun {
    pub guild: Option<Snowflake>,
    pub channel: Snowflake,
    pub session_id: String,
    pub command: SlashCommand,
    /// Option name, option kind and the value as the API wants it.
    pub answers: Vec<(String, u8, Value)>,
    pub nonce: String,
}

/// The request body for running a command.
pub fn run_body(run: &CommandRun) -> Value {
    let mut data = json!({
        "version": run.command.version,
        "id": run.command.id.to_string(),
        "name": run.command.root_name,
        "type": 1,
        "options": run.command.interaction_options(&run.answers),
        "application_command": run.command.root,
    });
    if let Some(guild) = run.guild {
        data["guild_id"] = json!(guild.to_string());
    }
    let mut body = json!({
        // Interaction type 2 is an application command.
        "type": 2,
        "application_id": run.command.application_id.to_string(),
        "channel_id": run.channel.to_string(),
        "session_id": run.session_id,
        "data": data,
        "nonce": run.nonce,
    });
    if let Some(guild) = run.guild {
        body["guild_id"] = json!(guild.to_string());
    }
    body
}

impl ApiClient {
    /// Every command usable in a channel.
    pub async fn command_index(&self, channel: Snowflake) -> Result<IndexOutcome, ApiError> {
        let response = self
            .send(
                Method::GET,
                &format!("/channels/{channel}/application-command-index"),
                "GET /channels/:id/application-command-index",
                None,
            )
            .await?;
        if response.status().as_u16() == 202 {
            return Ok(IndexOutcome::Pending);
        }
        let body: Value = serde_json::from_slice(&response.bytes().await?)?;
        Ok(parse_index(&body))
    }

    pub async fn run_command(&self, run: &CommandRun) -> Result<(), ApiError> {
        self.empty(Method::POST, "/interactions", "POST /interactions", Some(&run_body(run)))
            .await
    }
}

/// Reads the command index answer: the commands, flattened, and who owns them.
fn parse_index(body: &Value) -> IndexOutcome {
    let commands = body
        .get("application_commands")
        .and_then(Value::as_array)
        .map(|list| list.iter().flat_map(SlashCommand::flatten).collect())
        .unwrap_or_default();
    let apps = body
        .get("applications")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(|a| {
                    let id = a.get("id")?.as_str()?.parse().ok()?;
                    Some((Snowflake(id), a.get("name")?.as_str()?.to_owned()))
                })
                .collect()
        })
        .unwrap_or_default();
    IndexOutcome::Ready { commands, apps }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn command() -> SlashCommand {
        SlashCommand::flatten(&json!({
            "id": "10", "application_id": "20", "name": "echo", "description": "d", "version": "7",
            "options": [{ "type": 3, "name": "text", "required": true }]
        }))
        .remove(0)
    }

    #[test]
    fn the_index_lists_commands_and_their_apps() {
        let body = json!({
            "application_commands": [
                { "id": "1", "application_id": "20", "name": "ping", "description": "Pong" },
                { "id": "2", "application_id": "20", "name": "menu", "type": 2 }
            ],
            "applications": [{ "id": "20", "name": "Helper" }, { "name": "no id" }]
        });
        let IndexOutcome::Ready { commands, apps } = parse_index(&body) else { panic!("not ready") };
        assert_eq!(commands.len(), 1, "the context menu command is left out");
        assert_eq!(apps, vec![(Snowflake(20), "Helper".to_owned())]);
        let IndexOutcome::Ready { commands, .. } = parse_index(&json!({})) else { panic!() };
        assert!(commands.is_empty());
    }

    #[test]
    fn a_run_names_the_command_app_and_answers() {
        let run = CommandRun {
            guild: Some(Snowflake(9)),
            channel: Snowflake(5),
            session_id: "s".into(),
            command: command(),
            answers: vec![("text".into(), 3, json!("hello"))],
            nonce: "1".into(),
        };
        let body = run_body(&run);
        assert_eq!(body["type"], 2);
        assert_eq!(body["application_id"], "20");
        assert_eq!(body["guild_id"], "9");
        assert_eq!(body["data"]["id"], "10");
        assert_eq!(body["data"]["name"], "echo");
        assert_eq!(body["data"]["options"][0]["value"], "hello");
        assert_eq!(body["data"]["application_command"]["version"], "7");
    }

    #[test]
    fn direct_messages_have_no_guild() {
        let run = CommandRun {
            guild: None,
            channel: Snowflake(5),
            session_id: "s".into(),
            command: command(),
            answers: vec![],
            nonce: "1".into(),
        };
        let body = run_body(&run);
        assert!(body.get("guild_id").is_none() && body["data"].get("guild_id").is_none());
    }
}
