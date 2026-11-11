use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Preferences that outlive a session. The binary stores them on disk, the
/// UI only reads and changes this value.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Toasts for mentions and DMs while the window is in the background.
    pub desktop_notifications: bool,
    pub show_members: bool,
    /// How loud the microphone is sent in a call, 0 to 100 like the official
    /// client's Input Volume slider.
    pub input_volume: u8,
    /// How loud each person is played to us, 0 to 200, by user id. Anyone
    /// at the normal 100 is left out.
    pub user_volumes: BTreeMap<u64, u16>,
    /// Talk only while a key is held, instead of by voice activity.
    pub push_to_talk: bool,
    /// The talk key, named the way the window names a pressed key. Empty
    /// until one is picked.
    pub push_to_talk_key: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            desktop_notifications: true,
            show_members: true,
            input_volume: 100,
            user_volumes: BTreeMap::new(),
            push_to_talk: false,
            push_to_talk_key: String::new(),
        }
    }
}

impl Settings {
    /// The key to watch, once push to talk is on and a usable key is picked.
    pub fn talk_key(&self) -> Option<String> {
        let usable = crate::talk_key::virtual_key(&self.push_to_talk_key).is_some();
        (self.push_to_talk && usable).then(|| self.push_to_talk_key.clone())
    }

    /// How loud this person is played, as a percent.
    pub fn user_volume(&self, user: u64) -> u16 {
        self.user_volumes.get(&user).copied().unwrap_or(100)
    }

    /// Remembers a person's volume, forgetting it when it is back to normal.
    pub fn set_user_volume(&mut self, user: u64, percent: u16) {
        if percent == 100 {
            self.user_volumes.remove(&user);
        } else {
            self.user_volumes.insert(user, percent.min(200));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_fields_fall_back_to_defaults() {
        let settings: Settings = serde_json::from_str(r#"{"show_members":false}"#).unwrap();
        assert!(!settings.show_members);
        assert!(settings.desktop_notifications);
        assert_eq!(settings.input_volume, 100, "a file from before the slider means full volume");
        assert_eq!(serde_json::from_str::<Settings>("{}").unwrap(), Settings::default());
    }

    #[test]
    fn a_persons_volume_is_kept_only_while_it_is_not_normal() {
        let mut settings = Settings::default();
        assert_eq!(settings.user_volume(7), 100);
        settings.set_user_volume(7, 150);
        assert_eq!(settings.user_volume(7), 150);
        settings.set_user_volume(7, 900);
        assert_eq!(settings.user_volume(7), 200, "capped");
        settings.set_user_volume(7, 100);
        assert!(settings.user_volumes.is_empty());
    }

    #[test]
    fn push_to_talk_needs_both_the_switch_and_a_usable_key() {
        let mut settings = Settings::default();
        assert_eq!(settings.talk_key(), None, "off by default");
        settings.push_to_talk = true;
        assert_eq!(settings.talk_key(), None, "on, but no key picked: keep talking by voice");
        settings.push_to_talk_key = "v".into();
        assert_eq!(settings.talk_key().as_deref(), Some("v"));
        settings.push_to_talk_key = "escape".into();
        assert_eq!(settings.talk_key(), None, "a key that cannot be watched");
        settings.push_to_talk_key = "v".into();
        settings.push_to_talk = false;
        assert_eq!(settings.talk_key(), None);
    }

    #[test]
    fn volumes_survive_a_round_trip_through_the_file() {
        let mut settings = Settings::default();
        settings.set_user_volume(42, 30);
        let back: Settings = serde_json::from_str(&serde_json::to_string(&settings).unwrap()).unwrap();
        assert_eq!(back.user_volume(42), 30);
    }
}
