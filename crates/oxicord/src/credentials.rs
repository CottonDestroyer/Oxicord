//! The token lives in the OS credential store, never in a config file.

use keyring::Entry;

const SERVICE: &str = "oxicord";
const ACCOUNT: &str = "discord-token";

fn entry() -> keyring::Result<Entry> {
    Entry::new(SERVICE, ACCOUNT)
}

pub fn load() -> Option<String> {
    match entry().and_then(|e| e.get_password()) {
        Ok(token) => Some(token),
        Err(keyring::Error::NoEntry) => None,
        Err(err) => {
            // A broken keyring shouldn't stop the app, the user can still
            // paste the token by hand.
            tracing::warn!(%err, "could not read the saved token");
            None
        }
    }
}

pub fn save(token: &str) {
    if let Err(err) = entry().and_then(|e| e.set_password(token)) {
        tracing::warn!(%err, "could not save the token, you will need to sign in next time");
    }
}

pub fn clear() {
    match entry().and_then(|e| e.delete_credential()) {
        Ok(()) | Err(keyring::Error::NoEntry) => {}
        Err(err) => tracing::warn!(%err, "could not remove the saved token"),
    }
}
