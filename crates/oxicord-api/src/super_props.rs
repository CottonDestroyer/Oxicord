//! The client fingerprint Discord expects on every request and in the
//! gateway IDENTIFY. User accounts get flagged when it is missing or
//! inconsistent, so the REST client and the gateway share this one source.

use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};

/// Build number seen on the official web client when this was written. It
/// goes stale, so it is only a default.
pub const DEFAULT_BUILD_NUMBER: u32 = 663_810;

const BROWSER_VERSION: &str = "130.0.0.0";

#[derive(Clone, Debug)]
pub struct SuperProperties {
    pub build_number: u32,
    pub locale: String,
}

impl Default for SuperProperties {
    fn default() -> Self {
        Self {
            build_number: DEFAULT_BUILD_NUMBER,
            locale: "en-US".into(),
        }
    }
}

impl SuperProperties {
    pub fn user_agent(&self) -> String {
        format!(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
             (KHTML, like Gecko) Chrome/{BROWSER_VERSION} Safari/537.36"
        )
    }

    pub fn to_value(&self) -> Value {
        json!({
            "os": "Windows",
            "browser": "Chrome",
            "device": "",
            "system_locale": self.locale,
            "browser_user_agent": self.user_agent(),
            "browser_version": BROWSER_VERSION,
            "os_version": "10",
            "referrer": "",
            "referring_domain": "",
            "referrer_current": "",
            "referring_domain_current": "",
            "release_channel": "stable",
            "client_build_number": self.build_number,
            "client_event_source": null,
        })
    }

    /// Value for the `X-Super-Properties` header.
    pub fn to_header(&self) -> String {
        STANDARD.encode(self.to_value().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_round_trips_to_json() {
        let props = SuperProperties::default();
        let raw = STANDARD.decode(props.to_header()).unwrap();
        let v: Value = serde_json::from_slice(&raw).unwrap();
        assert_eq!(v["client_build_number"], DEFAULT_BUILD_NUMBER);
        assert_eq!(v["browser"], "Chrome");
    }

    #[test]
    fn user_agent_matches_reported_browser_version() {
        let ua = SuperProperties::default().user_agent();
        assert!(ua.contains(BROWSER_VERSION));
    }
}
