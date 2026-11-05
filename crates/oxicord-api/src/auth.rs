use std::fmt;

/// How we prove who we are. Only a pasted token is supported for now.
/// Email and password login will be another variant here, plus a function
/// that trades it for a token.
#[derive(Clone)]
pub enum Credentials {
    Token(String),
}

impl Credentials {
    pub(crate) fn authorization(&self) -> &str {
        match self {
            Self::Token(t) => t.trim(),
        }
    }
}

// Hand written so a stray `{:?}` in a log line can't leak the token.
impl fmt::Debug for Credentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Token(_) => f.write_str("Credentials::Token(..)"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_hides_the_token() {
        let c = Credentials::Token("secret-value".into());
        assert!(!format!("{c:?}").contains("secret"));
    }

    #[test]
    fn pasted_whitespace_is_trimmed() {
        let c = Credentials::Token("  abc\n".into());
        assert_eq!(c.authorization(), "abc");
    }
}
