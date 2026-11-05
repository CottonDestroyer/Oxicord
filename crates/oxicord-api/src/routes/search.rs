use std::time::Duration;

use oxicord_model::{Message, SearchPage, SearchQuery, Snowflake};
use reqwest::Method;
use serde_json::Value;

use crate::{ApiClient, ApiError};

/// Where a search looks.
#[derive(Clone, Copy, Debug)]
pub enum SearchScope {
    Guild(Snowflake),
    /// A direct message or group, which has no guild to search.
    Channel(Snowflake),
}

/// What a search request came back with.
#[derive(Debug)]
pub enum SearchOutcome {
    Ready(SearchPage),
    /// Discord is still building the index for this server and asks us to
    /// come back after the given wait.
    Indexing(Duration),
}

/// Page size Discord uses.
pub const SEARCH_PAGE: u32 = 25;

impl ApiClient {
    pub async fn search_messages(
        &self,
        scope: SearchScope,
        query: &SearchQuery,
        offset: u32,
    ) -> Result<SearchOutcome, ApiError> {
        let (base, route) = match scope {
            SearchScope::Guild(id) => (
                format!("/guilds/{id}/messages/search"),
                format!("GET /guilds/{id}/messages/search"),
            ),
            SearchScope::Channel(id) => (
                format!("/channels/{id}/messages/search"),
                format!("GET /channels/{id}/messages/search"),
            ),
        };
        let path = format!("{base}?{}", search_params(query, offset));
        let body: Value = self.json(Method::GET, &path, &route, None).await?;
        parse_search(&body)
    }

    /// Messages that mention us, newest first, for the inbox.
    pub async fn mentions(&self) -> Result<Vec<Message>, ApiError> {
        self.json(
            Method::GET,
            "/users/@me/mentions?limit=25&roles=true&everyone=true",
            "GET /users/@me/mentions",
            None,
        )
        .await
    }

    /// The pinned messages of a channel, newest pin first.
    pub async fn pins(&self, channel: Snowflake) -> Result<Vec<Message>, ApiError> {
        let body: Value = self
            .json(
                Method::GET,
                &format!("/channels/{channel}/messages/pins?limit=50"),
                &format!("GET /channels/{channel}/messages/pins"),
                None,
            )
            .await?;
        parse_pins(body)
    }

    pub async fn set_pinned(
        &self,
        channel: Snowflake,
        message: Snowflake,
        pinned: bool,
    ) -> Result<(), ApiError> {
        let method = if pinned { Method::PUT } else { Method::DELETE };
        self.empty(
            method.clone(),
            &format!("/channels/{channel}/messages/pins/{message}"),
            &format!("{method} /channels/{channel}/messages/pins/:id"),
            None,
        )
        .await
    }
}

/// The query string for a search: filters as repeated parameters, the way
/// Discord's own client sends them.
fn search_params(query: &SearchQuery, offset: u32) -> String {
    use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};

    let mut params = Vec::new();
    if !query.content.is_empty() {
        params.push(format!("content={}", utf8_percent_encode(&query.content, NON_ALPHANUMERIC)));
    }
    params.extend(query.authors.iter().map(|id| format!("author_id={id}")));
    params.extend(query.channels.iter().map(|id| format!("channel_id={id}")));
    params.extend(query.mentions.iter().map(|id| format!("mentions={id}")));
    params.extend(query.has.iter().map(|h| format!("has={}", utf8_percent_encode(h, NON_ALPHANUMERIC))));
    params.push(format!("limit={SEARCH_PAGE}"));
    params.push(format!("offset={offset}"));
    params.join("&")
}

/// The pins endpoint answers `{ items: [{ pinned_at, message }] }`, and the
/// older form was a bare array of messages. Accept both.
fn parse_pins(body: Value) -> Result<Vec<Message>, ApiError> {
    let messages = match body {
        Value::Array(list) => list,
        Value::Object(mut object) => match object.remove("items") {
            Some(Value::Array(items)) => items
                .into_iter()
                .filter_map(|mut item| item.get_mut("message").map(Value::take))
                .collect(),
            _ => Vec::new(),
        },
        _ => Vec::new(),
    };
    messages
        .into_iter()
        .map(|m| serde_json::from_value(m).map_err(ApiError::from))
        .collect()
}

/// Results come as groups of messages (a hit with its neighbours), and the
/// hit itself is marked. We only show the hit.
fn parse_search(body: &Value) -> Result<SearchOutcome, ApiError> {
    let Some(groups) = body.get("messages").and_then(Value::as_array) else {
        // Still indexing: no results, only a wait.
        let wait = body.get("retry_after").and_then(Value::as_f64).unwrap_or(2.0);
        return Ok(SearchOutcome::Indexing(Duration::from_secs_f64(wait.clamp(0.5, 10.0))));
    };

    let mut messages = Vec::new();
    for group in groups {
        let Some(list) = group.as_array() else { continue };
        let hit = list
            .iter()
            .find(|m| m.get("hit").and_then(Value::as_bool) == Some(true))
            .or_else(|| list.first());
        if let Some(hit) = hit {
            messages.push(serde_json::from_value(hit.clone())?);
        }
    }
    let total = body.get("total_results").and_then(Value::as_u64).unwrap_or(messages.len() as u64);
    Ok(SearchOutcome::Ready(SearchPage { total, messages }))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn message(id: &str, content: &str, hit: bool) -> Value {
        json!({
            "id": id, "channel_id": "5", "content": content, "hit": hit,
            "author": { "id": "1", "username": "ada" },
            "timestamp": "2024-05-01T12:00:00.000000+00:00", "type": 0
        })
    }

    #[test]
    fn filters_become_repeated_parameters() {
        let query = SearchQuery {
            content: "hello world".into(),
            authors: vec![Snowflake(5)],
            channels: vec![Snowflake(7), Snowflake(8)],
            mentions: vec![],
            has: vec!["image".into()],
        };
        assert_eq!(
            search_params(&query, 25),
            "content=hello%20world&author_id=5&channel_id=7&channel_id=8&has=image&limit=25&offset=25"
        );
        let only_filter = SearchQuery { has: vec!["link".into()], ..SearchQuery::default() };
        assert_eq!(search_params(&only_filter, 0), "has=link&limit=25&offset=0");
    }

    #[test]
    fn only_the_hit_of_each_group_is_kept() {
        let body = json!({
            "total_results": 40,
            "messages": [[message("1", "before", false), message("2", "match", true)]]
        });
        let SearchOutcome::Ready(page) = parse_search(&body).unwrap() else {
            panic!("expected results");
        };
        assert_eq!(page.total, 40);
        assert_eq!(page.messages.len(), 1);
        assert_eq!(page.messages[0].content, "match");
    }

    #[test]
    fn an_unready_index_asks_us_to_wait() {
        let body = json!({ "code": 110000, "retry_after": 3.0 });
        let SearchOutcome::Indexing(wait) = parse_search(&body).unwrap() else {
            panic!("expected a wait");
        };
        assert_eq!(wait, Duration::from_secs(3));
    }

    #[test]
    fn pins_accept_both_response_shapes() {
        let wrapped = json!({ "items": [{ "pinned_at": "x", "message": message("1", "a", false) }] });
        let bare = json!([message("2", "b", false)]);
        assert_eq!(parse_pins(wrapped).unwrap()[0].content, "a");
        assert_eq!(parse_pins(bare).unwrap()[0].content, "b");
        assert!(parse_pins(json!({})).unwrap().is_empty());
    }
}
