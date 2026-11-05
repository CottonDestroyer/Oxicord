//! The login lives in the OS credential store, never in a config file.
//!
//! It is the token plus, when we have one, the identity the token was issued
//! to. The two have to stay together: a token used with a different identity
//! gets revoked.

use keyring::Entry;
use oxicord_browser_login::Identity;
use serde::{Deserialize, Serialize};

const SERVICE: &str = "oxicord";
const ACCOUNT: &str = "discord-token";

/// Windows refuses credential blobs over 2560 bytes, keep clear of that.
const MAX_STORED_LEN: usize = 2400;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedLogin {
    pub token: String,
    pub identity: Option<Identity>,
}

fn entry() -> keyring::Result<Entry> {
    Entry::new(SERVICE, ACCOUNT)
}

fn encode(login: &SavedLogin) -> String {
    serde_json::to_string(login).expect("a login always serializes")
}

/// Reads the stored form. Early versions stored the bare token, so anything
/// that isn't a JSON object is taken to be one.
fn decode(text: &str) -> Option<SavedLogin> {
    let text = text.trim();
    if text.starts_with('{') {
        serde_json::from_str(text).ok()
    } else if text.is_empty() {
        None
    } else {
        Some(SavedLogin {
            token: text.to_owned(),
            identity: None,
        })
    }
}

pub fn load() -> Option<SavedLogin> {
    match entry().and_then(|e| e.get_password()) {
        Ok(text) => decode(&text),
        Err(keyring::Error::NoEntry) => None,
        Err(err) => {
            // A broken keyring shouldn't stop the app, the user can still
            // sign in again.
            tracing::warn!(%err, "could not read the saved login");
            None
        }
    }
}

pub fn save(token: &str, identity: Option<&Identity>) {
    let mut login = SavedLogin {
        token: token.to_owned(),
        identity: identity.cloned(),
    };
    let mut text = encode(&login);
    if text.len() > MAX_STORED_LEN {
        // Better to keep the token than to fail the save. The next launch
        // will present the default identity, which may not be accepted.
        tracing::warn!("login too large to store with its identity, storing the token alone");
        login.identity = None;
        text = encode(&login);
    }

    if let Err(err) = entry().and_then(|e| e.set_password(&text)) {
        tracing::warn!(%err, "could not save the login, you will need to sign in next time");
    }
}

pub fn clear() {
    match entry().and_then(|e| e.delete_credential()) {
        Ok(()) | Err(keyring::Error::NoEntry) => {}
        Err(err) => tracing::warn!(%err, "could not remove the saved login"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity() -> Identity {
        Identity {
            user_agent: "UA".into(),
            super_properties: "e30=".into(),
            locale: Some("en-GB".into()),
            timezone: None,
        }
    }

    #[test]
    fn a_login_survives_a_round_trip() {
        let login = SavedLogin {
            token: "tok".into(),
            identity: Some(identity()),
        };
        assert_eq!(decode(&encode(&login)), Some(login));
    }

    #[test]
    fn the_old_bare_token_format_still_loads() {
        let login = decode("  abc.def.ghi\n").unwrap();
        assert_eq!(login.token, "abc.def.ghi");
        assert_eq!(login.identity, None);
    }

    #[test]
    fn empty_or_corrupt_entries_load_as_nothing() {
        assert_eq!(decode(""), None);
        assert_eq!(decode("{ not json"), None);
    }
}
