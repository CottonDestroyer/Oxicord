use oxicord_model::Gif;
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use reqwest::Method;
use serde_json::Value;

use crate::{ApiClient, ApiError};

/// Results asked for per search, enough to fill the picker.
const LIMIT: u32 = 30;

impl ApiClient {
    /// What is popular right now.
    pub async fn gifs_trending(&self) -> Result<Vec<Gif>, ApiError> {
        let body: Value = self
            .json(
                Method::GET,
                &format!("/gifs/trending-gifs?locale=en-US&media_format=mp4&limit={LIMIT}"),
                "GET /gifs/trending-gifs",
                None,
            )
            .await?;
        // The answer is either `{ gifs: [...] }` or a bare list.
        Ok(Gif::list(body.get("gifs").unwrap_or(&body)))
    }

    pub async fn gifs_search(&self, query: &str) -> Result<Vec<Gif>, ApiError> {
        let q = utf8_percent_encode(query, NON_ALPHANUMERIC);
        let body: Value = self
            .json(
                Method::GET,
                &format!("/gifs/search?q={q}&media_format=mp4&locale=en-US&limit={LIMIT}"),
                "GET /gifs/search",
                None,
            )
            .await?;
        Ok(Gif::list(&body))
    }
}
