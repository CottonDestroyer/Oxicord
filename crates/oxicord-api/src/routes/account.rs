use oxicord_model::{AccountDetails, AuthorizedApp, Connection, LoginSession, Snowflake};
use oxicord_settings::{AccountSettings, Patch};
use reqwest::Method;
use serde::Deserialize;
use serde_json::json;

use crate::{ApiClient, ApiError};

#[derive(Deserialize)]
struct SettingsResponse {
    settings: String,
}

#[derive(Deserialize)]
struct PasswordAnswer {
    /// The sign-in token that replaces the one the old password made.
    #[serde(default)]
    token: Option<String>,
}

#[derive(Deserialize)]
struct AuthorizeResponse {
    url: String,
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

    /// Upload a new avatar (a base64 data URI) or clear it with `None`.
    pub async fn set_avatar(&self, avatar: Option<&str>) -> Result<(), ApiError> {
        let body = json!({ "avatar": avatar });
        self.empty(Method::PATCH, "/users/@me", "PATCH /users/@me", Some(&body))
            .await
    }

    /// Upload a new banner (a base64 data URI) or clear it with `None`.
    pub async fn set_banner(&self, banner: Option<&str>) -> Result<(), ApiError> {
        let body = json!({ "banner": banner });
        self.empty(Method::PATCH, "/users/@me", "PATCH /users/@me", Some(&body))
            .await
    }

    /// Put on an avatar decoration by sku, or take it off with `None`.
    pub async fn set_avatar_decoration(&self, sku: Option<&str>) -> Result<(), ApiError> {
        let body = decoration_body(sku);
        self.empty(Method::PATCH, "/users/@me", "PATCH /users/@me", Some(&body))
            .await
    }

    /// Put on a profile effect by sku, or take it off with `None`. The field
    /// name is the client's `profile_effect_sku_id`; never sent from the real
    /// account, so the shape is unit tested only.
    pub async fn set_profile_effect(&self, sku: Option<&str>) -> Result<(), ApiError> {
        let body = profile_effect_body(sku);
        self.empty(Method::PATCH, "/users/@me", "PATCH /users/@me", Some(&body))
            .await
    }

    /// Put on a profile frame by sku, or take it off with `None`. The field
    /// name is the client's `profile_frame_sku_id`; never sent from the real
    /// account, so the shape is unit tested only.
    pub async fn set_profile_frame(&self, sku: Option<&str>) -> Result<(), ApiError> {
        let body = profile_frame_body(sku);
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

    /// Where to send the person to link an outside account, such as
    /// `github`. The service sends them back to Discord's own callback page in
    /// their browser, which finishes the link there. Gated by the
    /// account-write rule: built and unit tested, never sent from the real
    /// account.
    pub async fn connection_authorize_url(&self, platform: &str) -> Result<String, ApiError> {
        let answer: AuthorizeResponse = self
            .json(
                Method::GET,
                &format!("/connections/{platform}/authorize"),
                "GET /connections/:platform/authorize",
                None,
            )
            .await?;
        Ok(answer.url)
    }

    /// Changes our password. The answer carries the token that replaces the
    /// old one, which the password change signs out. Gated by the account-write
    /// rule: built and unit tested, never sent from the real account.
    pub async fn change_password(&self, current: &str, new: &str) -> Result<Option<String>, ApiError> {
        let answer: PasswordAnswer = self
            .json(Method::PATCH, "/users/@me", "PATCH /users/@me", Some(&password_body(current, new)))
            .await?;
        Ok(answer.token)
    }

    /// Turns the authenticator app on: the key the person scanned and the
    /// first code it made. The answer carries the backup codes and the token
    /// that replaces the old one. Gated by the account-write rule: built and
    /// unit tested, never sent from the real account. The body is the web
    /// client's as far as its bundle shows; the lazy chunk that sends it was
    /// not captured, so the exact fields are unconfirmed.
    pub async fn enable_totp(&self, secret: &str, code: &str) -> Result<oxicord_model::TotpEnabled, ApiError> {
        self.json(
            Method::POST,
            "/users/@me/mfa/totp/enable",
            "POST /users/@me/mfa/totp/enable",
            Some(&totp_body(secret, code)),
        )
        .await
    }

    /// Removes a linked connection.
    pub async fn remove_connection(&self, kind: &str, id: &str) -> Result<(), ApiError> {
        self.empty(
            Method::DELETE,
            &format!("/users/@me/connections/{kind}/{id}"),
            "DELETE /users/@me/connections/:type/:id",
            None,
        )
        .await
    }

    /// Notification center items for the "For You" inbox tab.
    pub async fn notification_center_items(&self) -> Result<Vec<serde_json::Value>, ApiError> {
        self.json(
            Method::GET,
            "/users/@me/notification-center/items",
            "GET /users/@me/notification-center/items",
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

    /// Logs out every session except the current one.
    pub async fn logout_other_sessions(&self) -> Result<(), ApiError> {
        self.empty(Method::POST, "/auth/sessions/logout", "POST /auth/sessions/logout", None)
            .await
    }
}

fn totp_body(secret: &str, code: &str) -> serde_json::Value {
    json!({ "secret": secret, "code": code })
}

fn password_body(current: &str, new: &str) -> serde_json::Value {
    json!({ "password": current, "new_password": new })
}

fn decoration_body(sku: Option<&str>) -> serde_json::Value {
    json!({
        "avatar_decoration_data": sku.map(|sku| json!({ "sku_id": sku })),
    })
}

fn profile_effect_body(sku: Option<&str>) -> serde_json::Value {
    json!({ "profile_effect_sku_id": sku })
}

fn profile_frame_body(sku: Option<&str>) -> serde_json::Value {
    json!({ "profile_frame_sku_id": sku })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_effect_is_set_by_sku_and_cleared_with_null() {
        assert_eq!(
            profile_effect_body(Some("1139323098370424932")),
            json!({ "profile_effect_sku_id": "1139323098370424932" })
        );
        assert_eq!(profile_effect_body(None), json!({ "profile_effect_sku_id": null }));
    }

    #[test]
    fn a_frame_is_set_by_sku_and_cleared_with_null() {
        assert_eq!(
            profile_frame_body(Some("1556809649808736266")),
            json!({ "profile_frame_sku_id": "1556809649808736266" })
        );
        assert_eq!(profile_frame_body(None), json!({ "profile_frame_sku_id": null }));
    }

    #[test]
    fn a_password_change_names_both_passwords_and_returns_a_token() {
        assert_eq!(
            password_body("old", "new"),
            json!({ "password": "old", "new_password": "new" })
        );
        let answer: PasswordAnswer = serde_json::from_str(r#"{"id":"1","token":"fresh"}"#).unwrap();
        assert_eq!(answer.token.as_deref(), Some("fresh"));
        let none: PasswordAnswer = serde_json::from_str(r#"{"id":"1"}"#).unwrap();
        assert!(none.token.is_none());
    }

    #[test]
    fn turning_on_the_authenticator_sends_the_key_and_the_first_code() {
        assert_eq!(
            totp_body("NJYGYR6P", "123456"),
            json!({ "secret": "NJYGYR6P", "code": "123456" })
        );
    }

    #[test]
    fn the_authorize_answer_is_a_url_to_open() {
        let answer: AuthorizeResponse =
            serde_json::from_str(r#"{"url":"https://github.com/login/oauth/authorize?state=abc"}"#).unwrap();
        assert_eq!(answer.url, "https://github.com/login/oauth/authorize?state=abc");
    }

    #[test]
    fn a_decoration_is_set_by_sku_and_cleared_with_null() {
        assert_eq!(
            decoration_body(Some("7")),
            json!({ "avatar_decoration_data": { "sku_id": "7" } })
        );
        assert_eq!(decoration_body(None), json!({ "avatar_decoration_data": null }));
    }
}
