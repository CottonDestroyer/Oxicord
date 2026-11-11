//! The server welcome screen, `GET /guilds/:id/welcome-screen`.

use serde::Deserialize;

use crate::Snowflake;

/// Most channels the welcome screen can feature.
pub const MAX_WELCOME_CHANNELS: usize = 5;

/// What a newcomer sees before picking a channel.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct WelcomeScreen {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub welcome_channels: Vec<WelcomeChannel>,
}

impl WelcomeScreen {
    /// Features another channel, unless it is there already or the list is full.
    pub fn add_channel(&mut self, channel: Snowflake) -> bool {
        if self.welcome_channels.len() >= MAX_WELCOME_CHANNELS || self.welcome_channels.iter().any(|c| c.channel_id == channel) {
            return false;
        }
        self.welcome_channels.push(WelcomeChannel { channel_id: channel, description: String::new(), emoji_id: None, emoji_name: None });
        true
    }

    /// The `PATCH /guilds/:id/welcome-screen` body. An empty description goes
    /// as null, and a channel's emoji travels as the web client sends it.
    pub fn body(&self) -> serde_json::Value {
        let description = self.description.as_deref().map(str::trim).filter(|d| !d.is_empty());
        serde_json::json!({
            "enabled": self.enabled,
            "description": description,
            "welcome_channels": self.welcome_channels.iter().map(|c| serde_json::json!({
                "channel_id": c.channel_id.to_string(),
                "description": c.description.trim(),
                "emoji_id": c.emoji_id.map(|e| e.to_string()),
                "emoji_name": c.emoji_name,
            })).collect::<Vec<_>>(),
        })
    }
}

/// One featured channel of the welcome screen.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct WelcomeChannel {
    pub channel_id: Snowflake,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub emoji_id: Option<Snowflake>,
    #[serde(default)]
    pub emoji_name: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_scp_welcome_screen() {
        let text = include_str!("../../../fixtures/welcome-screen.json");
        let screen: WelcomeScreen = serde_json::from_str(text).unwrap();
        assert_eq!(
            screen.description.as_deref(),
            Some("The official communications server for the SCP: Roleplay Roblox game.")
        );
        assert_eq!(screen.welcome_channels.len(), 5);
        let first = &screen.welcome_channels[0];
        assert_eq!(first.channel_id, Snowflake(839_554_638_365_589_554));
        assert_eq!(first.description, "Read our server rules");
        assert_eq!(first.emoji_name.as_deref(), Some("\u{1f44f}"));
        let second = &screen.welcome_channels[1];
        assert_eq!(second.emoji_id, Some(Snowflake(700_478_921_128_411_196)));
        assert_eq!(second.emoji_name.as_deref(), Some("SCP"));
    }

    #[test]
    fn the_edit_body_carries_the_flag_the_text_and_each_channel() {
        let mut screen = WelcomeScreen { enabled: true, description: Some("  Hello  ".into()), ..WelcomeScreen::default() };
        assert!(screen.add_channel(Snowflake(7)));
        screen.welcome_channels[0].description = " Rules ".into();
        screen.welcome_channels[0].emoji_name = Some("\u{1f44f}".into());
        let body = screen.body();
        assert_eq!(body["enabled"], true);
        assert_eq!(body["description"], "Hello");
        assert_eq!(body["welcome_channels"][0]["channel_id"], "7");
        assert_eq!(body["welcome_channels"][0]["description"], "Rules");
        assert_eq!(body["welcome_channels"][0]["emoji_id"], serde_json::Value::Null);
        assert_eq!(body["welcome_channels"][0]["emoji_name"], "\u{1f44f}");

        screen.description = Some("   ".into());
        assert_eq!(screen.body()["description"], serde_json::Value::Null, "a blank description is cleared");
    }

    #[test]
    fn a_channel_is_featured_once_and_only_five_fit() {
        let mut screen = WelcomeScreen::default();
        assert!(screen.add_channel(Snowflake(1)));
        assert!(!screen.add_channel(Snowflake(1)), "no duplicates");
        for id in 2..=5 {
            assert!(screen.add_channel(Snowflake(id)));
        }
        assert!(!screen.add_channel(Snowflake(6)), "the sixth is refused");
        assert_eq!(screen.welcome_channels.len(), MAX_WELCOME_CHANNELS);
    }
}
