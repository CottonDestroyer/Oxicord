//! Sign in through Discord's own login page, in a temporary window.
//!
//! The user does the real login there, password, captcha, MFA or QR, so we
//! never handle any of it. When the page starts calling the API, we take the
//! token from its `Authorization` header and close the window.

mod handoff;
#[cfg(windows)]
mod window;

/// How the sign-in window ended.
#[derive(Debug)]
pub enum Outcome {
    Token(String),
    /// The user closed the window before finishing.
    Closed,
    /// Nothing happened for ten minutes.
    TimedOut,
    /// The window or webview could not be created, for example because the
    /// WebView2 runtime is missing. The text is meant for the user.
    Failed(String),
    /// This platform has no hosted login yet.
    Unsupported,
}

/// Opens the sign-in window and blocks until it ends. Run it on a thread of
/// its own, it owns that thread's message loop while the window is open.
pub fn sign_in() -> Outcome {
    #[cfg(windows)]
    {
        window::run()
    }
    #[cfg(not(windows))]
    {
        Outcome::Unsupported
    }
}
