use serde::{Deserialize, Serialize};

/// Preferences that outlive a session. The binary stores them on disk, the
/// UI only reads and changes this value.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Toasts for mentions and DMs while the window is in the background.
    pub desktop_notifications: bool,
    pub show_members: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            desktop_notifications: true,
            show_members: true,
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
        assert_eq!(serde_json::from_str::<Settings>("{}").unwrap(), Settings::default());
    }
}
