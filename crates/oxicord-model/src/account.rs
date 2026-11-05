//! Things the settings screens list: our own account details and the
//! places it is signed in or linked to.

use serde::Deserialize;

use crate::Snowflake;

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

/// An app we granted access to.
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

/// An outside account linked to ours, such as Steam or GitHub.
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
