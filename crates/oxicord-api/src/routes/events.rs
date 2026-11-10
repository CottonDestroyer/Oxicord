use oxicord_model::{EventUserCounts, Snowflake};
use reqwest::Method;

use crate::{ApiClient, ApiError};

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
}
