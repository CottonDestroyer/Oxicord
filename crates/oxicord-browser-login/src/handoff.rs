//! The pure half of the hosted login: what the page may do and how we read
//! what it sends back. Kept free of any webview types so it is testable.

use url::Url;

pub const LOGIN_URL: &str = "https://discord.com/login";

/// How long the sign-in window may stay open.
pub const TIMEOUT_SECS: u64 = 10 * 60;

const CAPABILITY_PLACEHOLDER: &str = "__OXICORD_CAPABILITY__";
const MIN_TOKEN_LEN: usize = 16;
const MAX_TOKEN_LEN: usize = 2048;

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

/// Reads a message from the page. Only a message that starts with this
/// session's secret capability counts, so other scripts running in the page
/// can't feed us a token of their choosing.
pub fn read_message<'a>(message: &'a str, capability: &str) -> Option<&'a str> {
    let token = message.strip_prefix(capability)?;
    is_plausible_token(token).then_some(token)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CAP: &str = "c0ffee";

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
    fn message_needs_the_capability_prefix() {
        let token = "mfa.abcdefghijklmnopqrstuvwxyz";
        assert_eq!(read_message(&format!("{CAP}{token}"), CAP), Some(token));
        assert_eq!(read_message(token, CAP), None);
        assert_eq!(read_message(&format!("wrong{token}"), CAP), None);
    }

    #[test]
    fn implausible_tokens_are_rejected() {
        assert!(!is_plausible_token("short"));
        assert!(!is_plausible_token("has whitespace inside the token value"));
        assert!(!is_plausible_token(&"a".repeat(MAX_TOKEN_LEN + 1)));
        assert!(is_plausible_token(&"a".repeat(MAX_TOKEN_LEN)));
    }

    #[test]
    fn script_embeds_the_capability_and_leaves_no_placeholder() {
        let js = script("deadbeef");
        assert!(js.contains("\"deadbeef\""));
        assert!(!js.contains(CAPABILITY_PLACEHOLDER));
    }
}
