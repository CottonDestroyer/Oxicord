//! The client identity presented to Discord on every request and in the
//! gateway IDENTIFY.
//!
//! Discord classifies user sessions by this fingerprint, and a mismatch
//! between transports, or a header that is present on one and missing on
//! another, gets the token revoked. The REST client and the gateway share
//! this one source so they can't drift apart.
//!
//! The values mirror a known working native client. They were chosen for
//! consistency, not for being the newest, so don't bump one without
//! checking the others.

use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};

/// Build number of the web client this identity imitates.
pub const DEFAULT_BUILD_NUMBER: u32 = 634_304;

const CHROME_MAJOR: &str = "128";
const SAFARI_VERSION: &str = "18.5";

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

/// Discord's spelling of the host operating system.
fn os_name() -> &'static str {
    match std::env::consts::OS {
        "macos" => "Mac OS X",
        "windows" => "Windows",
        _ => "Linux",
    }
}

fn platform_token() -> &'static str {
    match std::env::consts::OS {
        "macos" => "Macintosh; Intel Mac OS X 10_15_7",
        "windows" => "Windows NT 10.0; Win64; x64",
        _ => "X11; Linux x86_64",
    }
}

fn is_safari() -> bool {
    cfg!(target_os = "macos")
}

impl SuperProperties {
    pub fn browser(&self) -> &'static str {
        if is_safari() { "Safari" } else { "Chrome" }
    }

    pub fn browser_version(&self) -> String {
        if is_safari() {
            SAFARI_VERSION.to_owned()
        } else {
            format!("{CHROME_MAJOR}.0.0.0")
        }
    }

    pub fn user_agent(&self) -> String {
        let platform = platform_token();
        if is_safari() {
            format!(
                "Mozilla/5.0 ({platform}) AppleWebKit/605.1.15 (KHTML, like Gecko) \
                 Version/{SAFARI_VERSION} Safari/605.1.15"
            )
        } else {
            format!(
                "Mozilla/5.0 ({platform}) AppleWebKit/537.36 (KHTML, like Gecko) \
                 Chrome/{CHROME_MAJOR}.0.0.0 Safari/537.36"
            )
        }
    }

    pub fn to_value(&self) -> Value {
        json!({
            "os": os_name(),
            "browser": self.browser(),
            "device": "",
            "system_locale": self.locale,
            "browser_user_agent": self.user_agent(),
            "browser_version": self.browser_version(),
            "os_version": "",
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
        assert_eq!(v["os_version"], "");
        assert_eq!(v["browser"], props.browser());
    }

    #[test]
    fn user_agent_agrees_with_the_reported_browser() {
        let props = SuperProperties::default();
        let ua = props.user_agent();
        assert!(ua.starts_with("Mozilla/5.0 ("));
        assert_eq!(ua.contains("Chrome/"), props.browser() == "Chrome");
        assert_eq!(props.to_value()["browser_user_agent"], ua);
    }

    #[test]
    fn reported_version_matches_the_user_agent() {
        let props = SuperProperties::default();
        let version = props.browser_version();
        let major = version.split('.').next().unwrap();
        assert!(props.user_agent().contains(&format!("/{major}")));
    }
}
