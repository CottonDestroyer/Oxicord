use std::{sync::Arc, time::Duration};

use reqwest::{
    Method, Response,
    header::{
        ACCEPT_LANGUAGE, AUTHORIZATION, HeaderMap, HeaderName, HeaderValue, ORIGIN, REFERER,
        USER_AGENT,
    },
};
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::{
    ApiError, Credentials,
    ratelimit::{RateHeaders, RateLimiter},
    super_props::SuperProperties,
};

const BASE_URL: &str = "https://discord.com/api/v10";
const MAX_RETRIES: u32 = 3;

#[derive(Clone, Debug)]
pub struct ApiClient {
    http: reqwest::Client,
    limiter: Arc<RateLimiter>,
}

impl ApiClient {
    pub fn new(credentials: &Credentials) -> Result<Self, ApiError> {
        Self::with_props(credentials, &SuperProperties::default())
    }

    pub fn with_props(
        credentials: &Credentials,
        props: &SuperProperties,
    ) -> Result<Self, ApiError> {
        let mut headers = HeaderMap::new();

        let mut auth = HeaderValue::from_str(credentials.authorization())
            .map_err(|_| ApiError::BadToken)?;
        auth.set_sensitive(true);
        headers.insert(AUTHORIZATION, auth);

        headers.insert(
            HeaderName::from_static("x-super-properties"),
            HeaderValue::from_str(&props.to_header()).map_err(|_| ApiError::BadToken)?,
        );
        headers.insert(
            HeaderName::from_static("x-discord-locale"),
            HeaderValue::from_str(&props.locale).map_err(|_| ApiError::BadToken)?,
        );
        headers.insert(
            USER_AGENT,
            HeaderValue::from_str(&props.user_agent()).map_err(|_| ApiError::BadToken)?,
        );

        // The rest of what the web client sends on every call. A request
        // that has the super properties but lacks these looks scripted.
        headers.insert(
            HeaderName::from_static("x-discord-timezone"),
            HeaderValue::from_static("UTC"),
        );
        headers.insert(
            ACCEPT_LANGUAGE,
            HeaderValue::from_static("en-US,en;q=0.9"),
        );
        headers.insert(ORIGIN, HeaderValue::from_static("https://discord.com"));
        headers.insert(
            REFERER,
            HeaderValue::from_static("https://discord.com/channels/@me"),
        );

        let http = reqwest::Client::builder()
            .default_headers(headers)
            // An API call never legitimately redirects, so treat one as a
            // failure instead of following it somewhere else with our token.
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(30))
            .build()?;

        Ok(Self {
            http,
            limiter: Arc::default(),
        })
    }

    /// Sends a request and returns the raw response for 2xx statuses.
    ///
    /// `route` identifies the rate limit bucket. It is the method plus the
    /// path with the major id (channel or guild) kept and minor ids masked.
    pub(crate) async fn send(
        &self,
        method: Method,
        path: &str,
        route: &str,
        body: Option<&Value>,
    ) -> Result<Response, ApiError> {
        let url = format!("{BASE_URL}{path}");

        for _ in 0..=MAX_RETRIES {
            self.limiter.acquire(route).await;

            let mut req = self.http.request(method.clone(), &url);
            if let Some(body) = body {
                req = req.json(body);
            }
            let resp = req.send().await?;

            let status = resp.status();
            self.limiter
                .observe(route, &RateHeaders::from_headers(resp.headers()));

            if status.as_u16() == 429 {
                let (retry_after, global) = rate_limit_details(resp).await;
                tracing::warn!(route, ?retry_after, global, "rate limited, backing off");
                self.limiter.limited(route, retry_after, global);
                continue;
            }
            if !status.is_success() {
                let bytes = resp.bytes().await?;
                return Err(ApiError::from_body(status.as_u16(), &bytes));
            }
            return Ok(resp);
        }

        Err(ApiError::RateLimited(MAX_RETRIES))
    }

    pub(crate) async fn json<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        route: &str,
        body: Option<&Value>,
    ) -> Result<T, ApiError> {
        let resp = self.send(method, path, route, body).await?;
        let bytes = resp.bytes().await?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    /// For endpoints that answer 204 or a body we have no use for.
    pub(crate) async fn empty(
        &self,
        method: Method,
        path: &str,
        route: &str,
        body: Option<&Value>,
    ) -> Result<(), ApiError> {
        self.send(method, path, route, body).await.map(drop)
    }
}

/// A 429 carries the wait both in the JSON body and in `Retry-After`. The
/// body has sub-second precision, so prefer it.
async fn rate_limit_details(resp: Response) -> (Duration, bool) {
    let header_secs = resp
        .headers()
        .get("retry-after")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<f64>().ok());
    let global_header = resp.headers().contains_key("x-ratelimit-global");

    let body: Option<Value> = resp.json().await.ok();
    let secs = body
        .as_ref()
        .and_then(|b| b["retry_after"].as_f64())
        .or(header_secs)
        .unwrap_or(1.0);
    let global = body
        .as_ref()
        .and_then(|b| b["global"].as_bool())
        .unwrap_or(global_header);

    (Duration::from_secs_f64(secs.clamp(0.0, 60.0)), global)
}
