use serde::Deserialize;

use crate::{Permissions, Snowflake};

#[derive(Clone, Debug, Deserialize)]
pub struct Role {
    pub id: Snowflake,
    pub name: String,
    #[serde(default)]
    pub color: u32,
    #[serde(default)]
    pub hoist: bool,
    #[serde(default)]
    pub position: i32,
    #[serde(default)]
    pub permissions: Permissions,
    #[serde(default)]
    pub managed: bool,
    #[serde(default)]
    pub mentionable: bool,
    #[serde(default)]
    pub icon: Option<String>,
}
