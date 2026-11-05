//! Client side rate limiting.
//!
//! Discord groups routes into buckets and reports the state in response
//! headers. We learn the route to bucket mapping from those headers, wait
//! when a bucket is empty, and honour the global limit. The bookkeeping is
//! pure and takes the clock as an argument so it can be tested without
//! sleeping.

use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};

use reqwest::header::HeaderMap;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RateHeaders {
    pub bucket: Option<String>,
    pub remaining: Option<u32>,
    pub reset_after: Option<Duration>,
}

impl RateHeaders {
    pub fn from_headers(headers: &HeaderMap) -> Self {
        let text = |name: &str| headers.get(name).and_then(|v| v.to_str().ok());

        Self {
            bucket: text("x-ratelimit-bucket").map(str::to_owned),
            remaining: text("x-ratelimit-remaining").and_then(|v| v.parse().ok()),
            reset_after: text("x-ratelimit-reset-after")
                .and_then(|v| v.parse::<f64>().ok())
                .filter(|s| s.is_finite() && *s >= 0.0)
                .map(Duration::from_secs_f64),
        }
    }
}

struct Bucket {
    remaining: u32,
    reset_at: Instant,
}

#[derive(Default)]
struct State {
    route_buckets: HashMap<String, String>,
    buckets: HashMap<String, Bucket>,
    global_until: Option<Instant>,
}

impl State {
    /// Returns how long to wait before sending. `None` means go now, and the
    /// slot has been reserved.
    fn reserve(&mut self, route: &str, now: Instant) -> Option<Duration> {
        if let Some(until) = self.global_until {
            if until > now {
                return Some(until - now);
            }
            self.global_until = None;
        }

        let id = self.route_buckets.get(route)?;
        let bucket = self.buckets.get_mut(id)?;

        if bucket.reset_at <= now {
            // The window rolled over. We don't know the new allowance, so
            // forget the stale state and let the next response correct it.
            let id = id.clone();
            self.buckets.remove(&id);
            return None;
        }
        if bucket.remaining == 0 {
            return Some(bucket.reset_at - now);
        }
        bucket.remaining -= 1;
        None
    }

    fn observe(&mut self, route: &str, headers: &RateHeaders, now: Instant) {
        let (Some(id), Some(remaining), Some(reset_after)) =
            (&headers.bucket, headers.remaining, headers.reset_after)
        else {
            return;
        };

        self.route_buckets.insert(route.to_owned(), id.clone());
        self.buckets.insert(
            id.clone(),
            Bucket {
                remaining,
                reset_at: now + reset_after,
            },
        );
    }

    fn limited(&mut self, route: &str, retry_after: Duration, global: bool, now: Instant) {
        let until = now + retry_after;
        if global {
            self.global_until = Some(until);
        } else if let Some(bucket) = self
            .route_buckets
            .get(route)
            .and_then(|id| self.buckets.get_mut(id))
        {
            bucket.remaining = 0;
            bucket.reset_at = until;
        }
    }
}

#[derive(Default)]
pub struct RateLimiter {
    state: Mutex<State>,
}

// The state is a mutex over plain maps, nothing worth printing.
impl std::fmt::Debug for RateLimiter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RateLimiter")
    }
}

impl RateLimiter {
    /// Waits until a request on `route` is allowed to go out.
    pub async fn acquire(&self, route: &str) {
        loop {
            let wait = self.lock().reserve(route, Instant::now());
            match wait {
                None => return,
                Some(d) => tokio::time::sleep(d).await,
            }
        }
    }

    pub fn observe(&self, route: &str, headers: &RateHeaders) {
        self.lock().observe(route, headers, Instant::now());
    }

    pub fn limited(&self, route: &str, retry_after: Duration, global: bool) {
        self.lock().limited(route, retry_after, global, Instant::now());
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        // The state is plain data, so a poisoned lock is still usable.
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use reqwest::header::HeaderValue;

    use super::*;

    const ROUTE: &str = "POST /channels/1/messages";

    fn headers(bucket: &str, remaining: u32, reset_secs: u64) -> RateHeaders {
        RateHeaders {
            bucket: Some(bucket.into()),
            remaining: Some(remaining),
            reset_after: Some(Duration::from_secs(reset_secs)),
        }
    }

    #[test]
    fn unknown_route_is_not_delayed() {
        let mut s = State::default();
        assert_eq!(s.reserve(ROUTE, Instant::now()), None);
    }

    #[test]
    fn waits_once_the_bucket_is_empty() {
        let mut s = State::default();
        let t0 = Instant::now();
        s.observe(ROUTE, &headers("b", 2, 5), t0);

        assert_eq!(s.reserve(ROUTE, t0), None);
        assert_eq!(s.reserve(ROUTE, t0), None);
        assert_eq!(s.reserve(ROUTE, t0), Some(Duration::from_secs(5)));
    }

    #[test]
    fn bucket_refills_after_reset() {
        let mut s = State::default();
        let t0 = Instant::now();
        s.observe(ROUTE, &headers("b", 0, 5), t0);

        assert!(s.reserve(ROUTE, t0).is_some());
        assert_eq!(s.reserve(ROUTE, t0 + Duration::from_secs(6)), None);
    }

    #[test]
    fn routes_sharing_a_bucket_share_the_budget() {
        let mut s = State::default();
        let t0 = Instant::now();
        s.observe("GET a", &headers("shared", 1, 5), t0);
        s.observe("GET b", &headers("shared", 1, 5), t0);

        assert_eq!(s.reserve("GET a", t0), None);
        assert!(s.reserve("GET b", t0).is_some());
    }

    #[test]
    fn global_limit_blocks_every_route() {
        let mut s = State::default();
        let t0 = Instant::now();
        s.limited("anything", Duration::from_secs(3), true, t0);

        assert_eq!(s.reserve("GET other", t0), Some(Duration::from_secs(3)));
        assert_eq!(s.reserve("GET other", t0 + Duration::from_secs(4)), None);
    }

    #[test]
    fn a_429_empties_the_known_bucket() {
        let mut s = State::default();
        let t0 = Instant::now();
        s.observe(ROUTE, &headers("b", 4, 1), t0);
        s.limited(ROUTE, Duration::from_secs(2), false, t0);

        assert_eq!(s.reserve(ROUTE, t0), Some(Duration::from_secs(2)));
    }

    #[test]
    fn parses_headers() {
        let mut map = HeaderMap::new();
        map.insert("x-ratelimit-bucket", HeaderValue::from_static("abc"));
        map.insert("x-ratelimit-remaining", HeaderValue::from_static("3"));
        map.insert("x-ratelimit-reset-after", HeaderValue::from_static("1.5"));

        let parsed = RateHeaders::from_headers(&map);
        assert_eq!(parsed.bucket.as_deref(), Some("abc"));
        assert_eq!(parsed.remaining, Some(3));
        assert_eq!(parsed.reset_after, Some(Duration::from_millis(1500)));
    }

    #[test]
    fn ignores_nonsense_reset_values() {
        let mut map = HeaderMap::new();
        map.insert("x-ratelimit-reset-after", HeaderValue::from_static("-4"));
        assert_eq!(RateHeaders::from_headers(&map).reset_after, None);
    }
}
