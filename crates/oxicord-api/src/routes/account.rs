use oxicord_model::Snowflake;
use oxicord_settings::{AccountSettings, Patch};
use reqwest::Method;
use serde::Deserialize;
use serde_json::json;

use crate::{ApiClient, ApiError};

/// What `/users/@me` says about our own account beyond the public fields.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct AccountDetails {
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub phone: Option<String>,
    #[serde(default)]
    pub verified: bool,
    #[serde(default)]
    pub mfa_enabled: bool,
    #[serde(default)]
    pub bio: Option<String>,
    #[serde(default)]
    pub pronouns: Option<String>,
    #[serde(default)]
    pub global_name: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct AuthorizedApp {
    pub id: Snowflake,
    pub application: AppInfo,
    #[serde(default)]
    pub scopes: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct AppInfo {
    pub name: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Connection {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub verified: bool,
}

/// A place we are signed in.
#[derive(Clone, Debug, Deserialize)]
pub struct LoginSession {
    #[serde(default)]
    pub approx_last_used_time: Option<String>,
    #[serde(default)]
    pub client_info: ClientInfo,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ClientInfo {
    #[serde(default)]
    pub os: Option<String>,
    #[serde(default)]
    pub platform: Option<String>,
    #[serde(default)]
    pub location: Option<String>,
}

#[derive(Deserialize)]
struct SettingsResponse {
    settings: String,
}

#[derive(Deserialize)]
struct SessionsResponse {
    #[serde(default)]
    user_sessions: Vec<LoginSession>,
}

impl ApiClient {
    pub async fn account_details(&self) -> Result<AccountDetails, ApiError> {
        self.json(Method::GET, "/users/@me", "GET /users/@me", None).await
    }

    /// Our changeable display name.
    pub async fn set_display_name(&self, name: &str) -> Result<(), ApiError> {
        let body = json!({ "global_name": (!name.is_empty()).then_some(name) });
        self.empty(Method::PATCH, "/users/@me", "PATCH /users/@me", Some(&body))
            .await
    }

    pub async fn edit_profile(&self, bio: &str, pronouns: &str) -> Result<(), ApiError> {
        let body = json!({ "bio": bio, "pronouns": pronouns });
        self.empty(
            Method::PATCH,
            "/users/@me/profile",
            "PATCH /users/@me/profile",
            Some(&body),
        )
        .await
    }

    pub async fn account_settings(&self) -> Result<AccountSettings, ApiError> {
        let response: SettingsResponse = self
            .json(
                Method::GET,
                "/users/@me/settings-proto/1",
                "GET /users/@me/settings-proto/1",
                None,
            )
            .await?;
        AccountSettings::decode(&response.settings).map_err(|err| ApiError::Protocol(err.to_string()))
    }

    pub async fn update_account_settings(&self, patch: &Patch) -> Result<(), ApiError> {
        let body = json!({
            "settings": patch.settings,
            "required_data_version": patch.required_data_version,
        });
        self.empty(
            Method::PATCH,
            "/users/@me/settings-proto/1",
            "PATCH /users/@me/settings-proto/1",
            Some(&body),
        )
        .await
    }

    pub async fn authorized_apps(&self) -> Result<Vec<AuthorizedApp>, ApiError> {
        self.json(Method::GET, "/oauth2/tokens", "GET /oauth2/tokens", None)
            .await
    }

    pub async fn revoke_app(&self, token: Snowflake) -> Result<(), ApiError> {
        self.empty(
            Method::DELETE,
            &format!("/oauth2/tokens/{token}"),
            "DELETE /oauth2/tokens/:id",
            None,
        )
        .await
    }

    pub async fn connections(&self) -> Result<Vec<Connection>, ApiError> {
        self.json(
            Method::GET,
            "/users/@me/connections",
            "GET /users/@me/connections",
            None,
        )
        .await
    }

    pub async fn login_sessions(&self) -> Result<Vec<LoginSession>, ApiError> {
        let response: SessionsResponse = self
            .json(Method::GET, "/auth/sessions", "GET /auth/sessions", None)
            .await?;
        Ok(response.user_sessions)
    }
}
