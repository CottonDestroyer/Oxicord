//! Opens the hosted sign-in window on its own and prints how it ended. The
//! token itself is never printed.
//!
//!     cargo run -p oxicord-browser-login --example try_sign_in

use oxicord_browser_login::Outcome;

fn main() {
    match oxicord_browser_login::sign_in() {
        Outcome::SignedIn(c) => println!(
            "got a token ({} characters), identity captured: {}",
            c.token.len(),
            c.identity.is_some()
        ),
        other => println!("ended without a token: {other:?}"),
    }
}
