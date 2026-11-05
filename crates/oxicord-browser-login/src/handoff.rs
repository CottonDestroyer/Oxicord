//! The pure half of the hosted login: what the page may do and how we read
//! what it sends back. Kept free of any webview types so it is testable.

use serde::{Deserialize, Serialize};
use url::Url;

pub const LOGIN_URL: &str = "https://discord.com/login";

/// How long the sign-in window may stay open.
pub const TIMEOUT_SECS: u64 = 10 * 60;

const CAPABILITY_PLACEHOLDER: &str = "__OXICORD_CAPABILITY__";
const MIN_TOKEN_LEN: usize = 16;
const MAX_TOKEN_LEN: usize = 2048;
const MAX_SUPER_PROPERTIES_LEN: usize = 8192;
const MAX_SHORT_FIELD_LEN: usize = 512;

/// The identity the sign-in page presented to Discord.
///
/// A token only keeps working when it is used with the identity it was
/// issued to, so the session that follows has to repeat this exactly.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Identity {
    pub user_agent: String,
    /// The `X-Super-Properties` header value, base64, byte for byte.
    pub super_properties: String,
    pub locale: Option<String>,
    pub timezone: Option<String>,
}

/// What the page reported once the user was signed in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Capture {
    pub token: String,
    /// Missing when the page never showed us its identity.
    pub identity: Option<Identity>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawCapture {
    token: String,
    super_properties: Option<String>,
    user_agent: Option<String>,
    locale: Option<String>,
    timezone: Option<String>,
}

/// The script that watches the page's own API calls for a token.
pub fn script(capability: &str) -> String {
    include_str!("handoff.js").replace(CAPABILITY_PLACEHOLDER, capability)
}

/// The page is only ever allowed to be Discord over HTTPS. Anything else,
/// a redirect included, is refused so a hostile page never sees our script.
pub fn is_discord_page(page: &str) -> bool {
    Url::parse(page)
        .map(|u| u.scheme() == "https" && u.host_str() == Some("discord.com") && u.port().is_none())
        .unwrap_or(false)
}

/// Same shape check the script applies. It is repeated here because the
/// page is not trusted to have run it faithfully.
pub fn is_plausible_token(value: &str) -> bool {
    (MIN_TOKEN_LEN..=MAX_TOKEN_LEN).contains(&value.len()) && !value.chars().any(char::is_whitespace)
}

fn is_header_safe(value: &str, max: usize) -> bool {
    !value.is_empty() && value.len() <= max && value.chars().all(|c| (' '..='~').contains(&c))
}

fn is_base64(value: &str) -> bool {
    value
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'='))
}

/// Reads a message from the page. Only a message that starts with this
/// session's secret capability counts, so other scripts running in the page
/// can't feed us a token of their choosing.
pub fn read_message(message: &str, capability: &str) -> Option<Capture> {
    let body = message.strip_prefix(capability)?;
    let raw: RawCapture = serde_json::from_str(body).ok()?;
    if !is_plausible_token(&raw.token) {
        return None;
    }

    // An identity is only usable whole. A half-reported one would be worse
    // than none, so drop it unless the essentials check out.
    let identity = match (raw.super_properties, raw.user_agent) {
        (Some(props), Some(agent))
            if is_header_safe(&props, MAX_SUPER_PROPERTIES_LEN)
                && is_base64(&props)
                && is_header_safe(&agent, MAX_SHORT_FIELD_LEN) =>
        {
            Some(Identity {
                user_agent: agent,
                super_properties: props,
                locale: raw.locale.filter(|v| is_header_safe(v, 64)),
                timezone: raw.timezone.filter(|v| is_header_safe(v, 64)),
            })
        }
        _ => None,
    };

    Some(Capture {
        token: raw.token,
        identity,
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    const CAP: &str = "c0ffee";
    const TOKEN: &str = "abcdefghijklmnopqrstuvwx.yzABCD.0123456789abcdefghijklmnopqrstuvwxyz";

    fn message(body: serde_json::Value) -> String {
        format!("{CAP}{body}")
    }

    #[test]
    fn only_https_discord_com_is_allowed() {
        assert!(is_discord_page("https://discord.com/login"));
        assert!(is_discord_page("https://discord.com/channels/@me"));
        assert!(!is_discord_page("http://discord.com/login"));
        assert!(!is_discord_page("https://discord.com.evil.example/login"));
        assert!(!is_discord_page("https://evil.example/discord.com"));
        assert!(!is_discord_page("https://discord.com:8443/login"));
        assert!(!is_discord_page("https://user@discord.com@evil.example/"));
        assert!(!is_discord_page("about:blank"));
        assert!(!is_discord_page("not a url"));
    }

    #[test]
    fn reads_the_token_and_the_identity_together() {
        let capture = read_message(
            &message(json!({
                "token": TOKEN,
                "superProperties": "eyJvcyI6IldpbmRvd3MifQ==",
                "userAgent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) Chrome/142.0.0.0",
                "locale": "en-GB",
                "timezone": "Europe/London"
            })),
            CAP,
        )
        .unwrap();

        assert_eq!(capture.token, TOKEN);
        let identity = capture.identity.unwrap();
        assert_eq!(identity.super_properties, "eyJvcyI6IldpbmRvd3MifQ==");
        assert_eq!(identity.locale.as_deref(), Some("en-GB"));
        assert_eq!(identity.timezone.as_deref(), Some("Europe/London"));
    }

    #[test]
    fn a_token_without_an_identity_is_still_accepted() {
        let capture = read_message(
            &message(json!({ "token": TOKEN, "superProperties": null, "userAgent": "UA" })),
            CAP,
        )
        .unwrap();
        assert_eq!(capture.token, TOKEN);
        assert_eq!(capture.identity, None);
    }

    #[test]
    fn a_half_reported_identity_is_dropped_not_kept() {
        let no_agent = read_message(
            &message(json!({ "token": TOKEN, "superProperties": "e30=" })),
            CAP,
        )
        .unwrap();
        assert_eq!(no_agent.identity, None);

        let not_base64 = read_message(
            &message(json!({ "token": TOKEN, "superProperties": "not base64!", "userAgent": "UA" })),
            CAP,
        )
        .unwrap();
        assert_eq!(not_base64.identity, None);
    }

    #[test]
    fn header_injection_in_the_identity_is_refused() {
        let capture = read_message(
            &message(json!({
                "token": TOKEN, "superProperties": "e30=",
                "userAgent": "UA\r\nX-Evil: 1"
            })),
            CAP,
        )
        .unwrap();
        assert_eq!(capture.identity, None);
    }

    #[test]
    fn message_needs_the_capability_prefix() {
        let body = json!({ "token": TOKEN }).to_string();
        assert!(read_message(&format!("{CAP}{body}"), CAP).is_some());
        assert!(read_message(&body, CAP).is_none());
        assert!(read_message(&format!("wrong{body}"), CAP).is_none());
    }

    #[test]
    fn implausible_tokens_are_rejected() {
        for token in ["short", "has whitespace inside the token value"] {
            assert!(read_message(&message(json!({ "token": token })), CAP).is_none());
        }
        assert!(!is_plausible_token(&"a".repeat(MAX_TOKEN_LEN + 1)));
        assert!(is_plausible_token(&"a".repeat(MAX_TOKEN_LEN)));
    }

    #[test]
    fn garbage_after_the_capability_is_ignored() {
        assert!(read_message(&format!("{CAP}not json"), CAP).is_none());
    }

    #[test]
    fn script_embeds_the_capability_and_leaves_no_placeholder() {
        let js = script("deadbeef");
        assert!(js.contains("\"deadbeef\""));
        assert!(!js.contains(CAPABILITY_PLACEHOLDER));
    }
}
