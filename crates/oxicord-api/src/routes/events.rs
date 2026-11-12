use oxicord_model::{EventRsvp, EventUserCounts, NewEvent, Snowflake};
use reqwest::Method;
use serde_json::{Value, json};

use crate::{ApiClient, ApiError};

/// The `response` the web client sends for "Interested".
const INTERESTED: u8 = 1;

impl ApiClient {
    /// How many people are interested in an event.
    pub async fn event_user_count(&self, guild: Snowflake, event: Snowflake) -> Result<u64, ApiError> {
        let counts: EventUserCounts = self
            .json(
                Method::GET,
                &format!("/guilds/{guild}/scheduled-events/{event}/users/counts"),
                "GET /guilds/:id/scheduled-events/:id/users/counts",
                None,
            )
            .await?;
        Ok(counts.guild_scheduled_event_count)
    }

    /// The events in a server that we have answered.
    pub async fn own_event_rsvps(&self, guild: Snowflake) -> Result<Vec<EventRsvp>, ApiError> {
        self.json(
            Method::GET,
            &format!("/users/@me/scheduled-events?guild_ids={guild}"),
            "GET /users/@me/scheduled-events",
            None,
        )
        .await
    }

    /// Marks us interested in an event, or takes it back. Gated by the
    /// account-write rule: built and unit tested, never sent from the real
    /// account.
    pub async fn set_event_interest(
        &self,
        guild: Snowflake,
        event: Snowflake,
        interested: bool,
    ) -> Result<(), ApiError> {
        let path = format!("/guilds/{guild}/scheduled-events/{event}/users/@me");
        if interested {
            self.empty(
                Method::PUT,
                &path,
                "PUT /guilds/:id/scheduled-events/:id/users/@me",
                Some(&interest_body()),
            )
            .await
        } else {
            self.empty(Method::DELETE, &path, "DELETE /guilds/:id/scheduled-events/:id/users/@me", None)
                .await
        }
    }
}

impl ApiClient {
    /// Schedules a new event. Gated by the account-write rule: built and unit
    /// tested, never sent from the real account.
    pub async fn create_scheduled_event(&self, guild: Snowflake, event: &NewEvent) -> Result<(), ApiError> {
        self.empty(
            Method::POST,
            &format!("/guilds/{guild}/scheduled-events"),
            "POST /guilds/:id/scheduled-events",
            Some(&event.body()),
        )
        .await
    }
}

fn interest_body() -> Value {
    json!({ "response": INTERESTED })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interest_is_a_response_of_one() {
        assert_eq!(interest_body().to_string(), r#"{"response":1}"#);
    }
}
