//! REST client for the Discord API.

mod auth;
mod client;
mod error;
pub mod ratelimit;
pub mod routes;
pub mod super_props;

pub use auth::Credentials;
pub use client::ApiClient;
pub use error::ApiError;
