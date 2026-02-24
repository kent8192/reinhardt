//! auth application module
//!
//! User authentication models for examples-twitter

use reinhardt::app_config;

pub mod models;
pub mod shared;
pub mod urls;

#[cfg(client)]
pub mod client;

#[cfg(server)]
pub mod server;

#[cfg(test)]
pub mod tests;

#[app_config(name = "auth", label = "auth", verbose_name = "Authentication")]
pub struct AuthConfig;
