//! auth application module
//!
//! User authentication models for examples-twitter

#[cfg(server)]
use reinhardt::app_config;

#[cfg(server)]
pub mod models;
pub mod shared;
pub mod urls;

#[cfg(client)]
pub mod client;

#[cfg(server)]
pub mod server;

#[cfg(test)]
pub mod tests;

#[cfg(server)]
#[app_config(name = "auth", label = "auth", verbose_name = "Authentication")]
pub struct AuthConfig;
