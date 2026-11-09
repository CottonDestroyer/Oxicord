//! The server welcome screen, `GET /guilds/:id/welcome-screen`.

use serde::Deserialize;

use crate::Snowflake;

/// What a newcomer sees before picking a channel.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct WelcomeScreen {
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub welcome_channels: Vec<WelcomeChannel>,
}

/// One featured channel of the welcome screen.
#[derive(Clone, Debug, Deserialize)]
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
}
