use std::{sync::Arc, time::Duration};

use tokio::sync::Semaphore;
use url::Url;

/// Larger than any avatar, emoji or inline preview we show. A bigger body
/// is dropped instead of held in memory.
const MAX_BYTES: usize = 8 * 1024 * 1024;

/// How many downloads may run at once. A busy channel asks for dozens of
/// avatars in one frame.
const CONCURRENCY: usize = 8;

#[derive(Debug, thiserror::Error)]
pub enum FetchError {
    #[error("refusing to fetch {0}")]
    NotAllowed(String),
    #[error("request failed: {0}")]
    Transport(#[from] reqwest::Error),
    #[error("server answered {0}")]
    Status(u16),
    #[error("image is larger than {MAX_BYTES} bytes")]
    TooLarge,
    #[error("response is not an image we can show")]
    NotAnImage,
}

/// Only Discord's own CDN and media proxy, over HTTPS. Embeds point at
/// arbitrary sites, but their previews are served through Discord's proxy,
/// so there is never a reason to contact a third party directly.
pub fn is_allowed(url: &str) -> bool {
    let Ok(url) = Url::parse(url) else {
        return false;
    };
    let Some(host) = url.host_str() else {
        return false;
    };
    // Discord's own CDN, plus the services behind the GIF picker.
    let discord_host = ["discordapp.com", "discordapp.net", "tenor.com", "giphy.com", "klipy.com"]
        .iter()
        .any(|domain| host == *domain || host.ends_with(&format!(".{domain}")));

    url.scheme() == "https" && url.port().is_none() && discord_host
}

/// Downloads images. It sends no credentials, the CDN needs none and our
/// token has no business there.
#[derive(Clone)]
pub struct Fetcher {
    client: reqwest::Client,
    permits: Arc<Semaphore>,
}

impl Fetcher {
    pub fn new() -> Result<Self, FetchError> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(20))
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        Ok(Self {
            client,
            permits: Arc::new(Semaphore::new(CONCURRENCY)),
        })
    }

    pub async fn fetch(&self, url: &str) -> Result<Vec<u8>, FetchError> {
        if !is_allowed(url) {
            return Err(FetchError::NotAllowed(url.to_owned()));
        }
        let _permit = self.permits.acquire().await.expect("semaphore is never closed");

        let mut response = self.client.get(url).send().await?;
        if !response.status().is_success() {
            return Err(FetchError::Status(response.status().as_u16()));
        }
        if response.content_length().is_some_and(|n| n > MAX_BYTES as u64) {
            return Err(FetchError::TooLarge);
        }

        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await? {
            if body.len() + chunk.len() > MAX_BYTES {
                return Err(FetchError::TooLarge);
            }
            body.extend_from_slice(&chunk);
        }

        crate::sniff(&body).ok_or(FetchError::NotAnImage)?;
        Ok(body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_discords_own_hosts_over_https() {
        assert!(is_allowed("https://cdn.discordapp.com/avatars/1/a.png"));
        assert!(is_allowed("https://media.discordapp.net/attachments/1/2/a.png"));
        assert!(is_allowed("https://images-ext-1.discordapp.net/external/x/https/a.png"));
    }

    #[test]
    fn refuses_everything_else() {
        assert!(!is_allowed("http://cdn.discordapp.com/a.png"));
        assert!(!is_allowed("https://evil.example/a.png"));
        assert!(!is_allowed("https://discordapp.com.evil.example/a.png"));
        assert!(!is_allowed("https://notdiscordapp.com/a.png"));
        assert!(!is_allowed("https://cdn.discordapp.com:8443/a.png"));
        assert!(!is_allowed("file:///c:/secret.png"));
        assert!(!is_allowed("nonsense"));
    }
}
