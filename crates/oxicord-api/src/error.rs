use serde::Deserialize;

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("request failed: {0}")]
    Transport(#[from] reqwest::Error),

    #[error("discord returned {status}: {message}")]
    Status {
        status: u16,
        /// Discord's own error code, when the body carried one.
        code: Option<u32>,
        message: String,
    },

    #[error("could not decode response: {0}")]
    Decode(#[from] serde_json::Error),

    #[error("unexpected settings data: {0}")]
    Protocol(String),

    #[error("still rate limited after {0} retries")]
    RateLimited(u32),

    #[error("invalid token: it contains characters that can't go in a header")]
    BadToken,

    #[error("the client identity contains characters that can't go in a header")]
    BadIdentity,
}

impl ApiError {
    /// True when the credentials were rejected, as opposed to a transient
    /// failure. The UI uses this to decide between "retry" and "log in again".
    pub fn is_unauthorized(&self) -> bool {
        matches!(self, Self::Status { status: 401, .. })
    }

    pub(crate) fn from_body(status: u16, body: &[u8]) -> Self {
        #[derive(Deserialize)]
        struct Body {
            code: Option<u32>,
            message: Option<String>,
        }

        match serde_json::from_slice::<Body>(body) {
            Ok(b) => Self::Status {
                status,
                code: b.code,
                message: b.message.unwrap_or_default(),
            },
            // HTML error pages from the CDN in front of the API end up here.
            Err(_) => Self::Status {
                status,
                code: None,
                message: String::from_utf8_lossy(body).chars().take(200).collect(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_discord_error_body() {
        let err = ApiError::from_body(403, br#"{"code": 50013, "message": "Missing Permissions"}"#);
        match err {
            ApiError::Status { status, code, message } => {
                assert_eq!(status, 403);
                assert_eq!(code, Some(50013));
                assert_eq!(message, "Missing Permissions");
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn falls_back_to_raw_text_for_non_json() {
        let err = ApiError::from_body(502, b"<html>Bad gateway</html>");
        assert!(err.to_string().contains("Bad gateway"));
    }

    #[test]
    fn only_401_counts_as_unauthorized() {
        assert!(ApiError::from_body(401, b"{}").is_unauthorized());
        assert!(!ApiError::from_body(403, b"{}").is_unauthorized());
    }
}
