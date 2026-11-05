//! The client identity presented to Discord on every request and in the
//! gateway IDENTIFY.
//!
//! Discord ties a token to the identity it was issued to, and revokes tokens
//! that show up with a different one. So there are two sources:
//!
//! - [`SuperProperties::from_browser`], the identity of the sign-in window
//!   that minted the token, repeated exactly. This is the one that matters.
//! - [`SuperProperties::default`], a fixed Chrome-on-Windows identity used
//!   when we have nothing better, for example a pasted token.
//!
//! The REST client and the gateway share one value so they can't drift apart.

use base64::{
    Engine,
    engine::general_purpose::{STANDARD, STANDARD_NO_PAD},
};
use serde_json::{Value, json};

/// Build number of the web client the fallback identity imitates.
pub const DEFAULT_BUILD_NUMBER: u32 = 634_304;

const CHROME_MAJOR: &str = "128";
const SAFARI_VERSION: &str = "18.5";

#[derive(Clone, Debug)]
pub struct SuperProperties {
    /// The `X-Super-Properties` header value, exactly as it is sent.
    header: String,
    /// The same thing decoded, for the gateway IDENTIFY.
    value: Value,
    user_agent: String,
    locale: String,
    timezone: String,
}

#[derive(Debug, thiserror::Error)]
pub enum IdentityError {
    #[error("super properties are not valid base64")]
    NotBase64,
    #[error("super properties are not a JSON object")]
    NotAnObject,
}

impl Default for SuperProperties {
    fn default() -> Self {
        let user_agent = default_user_agent();
        let value = json!({
            "os": os_name(),
            "browser": if is_safari() { "Safari" } else { "Chrome" },
            "device": "",
            "system_locale": "en-US",
            "browser_user_agent": user_agent,
            "browser_version": if is_safari() {
                SAFARI_VERSION.to_owned()
            } else {
                format!("{CHROME_MAJOR}.0.0.0")
            },
            "os_version": "",
            "referrer": "",
            "referring_domain": "",
            "referrer_current": "",
            "referring_domain_current": "",
            "release_channel": "stable",
            "client_build_number": DEFAULT_BUILD_NUMBER,
            "client_event_source": null,
        });
        Self {
            header: STANDARD.encode(value.to_string()),
            value,
            user_agent,
            locale: "en-US".into(),
            timezone: "UTC".into(),
        }
    }
}

impl SuperProperties {
    /// Adopts an identity captured from a real browser. The header is kept
    /// byte for byte, re-encoding it could reorder keys.
    pub fn from_browser(
        user_agent: &str,
        super_properties: &str,
        locale: Option<&str>,
        timezone: Option<&str>,
    ) -> Result<Self, IdentityError> {
        let raw = STANDARD
            .decode(super_properties)
            .or_else(|_| STANDARD_NO_PAD.decode(super_properties))
            .map_err(|_| IdentityError::NotBase64)?;
        let value: Value = serde_json::from_slice(&raw).map_err(|_| IdentityError::NotAnObject)?;
        if !value.is_object() {
            return Err(IdentityError::NotAnObject);
        }

        Ok(Self {
            header: super_properties.to_owned(),
            value,
            user_agent: user_agent.to_owned(),
            locale: locale.unwrap_or("en-US").to_owned(),
            timezone: timezone.unwrap_or("UTC").to_owned(),
        })
    }

    pub fn user_agent(&self) -> &str {
        &self.user_agent
    }

    pub fn locale(&self) -> &str {
        &self.locale
    }

    pub fn timezone(&self) -> &str {
        &self.timezone
    }

    /// Value for the `X-Super-Properties` header.
    pub fn to_header(&self) -> &str {
        &self.header
    }

    /// The decoded form, for the gateway IDENTIFY `properties`.
    pub fn to_value(&self) -> Value {
        self.value.clone()
    }

    pub fn build_number(&self) -> Option<u64> {
        self.value["client_build_number"].as_u64()
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

fn default_user_agent() -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_identity_is_internally_consistent() {
        let props = SuperProperties::default();
        let decoded: Value = serde_json::from_slice(&STANDARD.decode(props.to_header()).unwrap()).unwrap();

        assert_eq!(decoded, props.to_value());
        assert_eq!(decoded["browser_user_agent"], props.user_agent());
        assert_eq!(decoded["os_version"], "");
        assert_eq!(props.build_number(), Some(u64::from(DEFAULT_BUILD_NUMBER)));
        assert!(props.user_agent().starts_with("Mozilla/5.0 ("));
    }

    #[test]
    fn browser_identity_is_kept_exactly() {
        // Deliberately not what `serde_json` would produce, to prove the
        // header is passed through untouched.
        let header = STANDARD.encode(r#"{"os":"Windows","client_build_number":999999, "browser":"Edge"}"#);
        let props = SuperProperties::from_browser(
            "Mozilla/5.0 Test Edg/142",
            &header,
            Some("en-GB"),
            Some("Europe/London"),
        )
        .unwrap();

        assert_eq!(props.to_header(), header);
        assert_eq!(props.user_agent(), "Mozilla/5.0 Test Edg/142");
        assert_eq!(props.locale(), "en-GB");
        assert_eq!(props.timezone(), "Europe/London");
        assert_eq!(props.build_number(), Some(999_999));
        assert_eq!(props.to_value()["browser"], "Edge");
    }

    #[test]
    fn missing_locale_and_timezone_fall_back() {
        let header = STANDARD.encode("{}");
        let props = SuperProperties::from_browser("UA", &header, None, None).unwrap();
        assert_eq!((props.locale(), props.timezone()), ("en-US", "UTC"));
    }

    #[test]
    fn unpadded_base64_is_accepted() {
        let header = STANDARD_NO_PAD.encode(r#"{"a":1}"#);
        assert!(SuperProperties::from_browser("UA", &header, None, None).is_ok());
    }

    #[test]
    fn rejects_identities_that_are_not_an_object() {
        let list = STANDARD.encode("[1,2]");
        assert!(matches!(
            SuperProperties::from_browser("UA", &list, None, None),
            Err(IdentityError::NotAnObject)
        ));
        assert!(matches!(
            SuperProperties::from_browser("UA", "!!!", None, None),
            Err(IdentityError::NotBase64)
        ));
    }
}
