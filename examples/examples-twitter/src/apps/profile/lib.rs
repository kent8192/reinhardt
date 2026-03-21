//! profile application module
//!
//! User profile models for examples-twitter

#[cfg(server)]
use reinhardt::app_config;

#[cfg(server)]
pub mod admin;
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
#[app_config(name = "profile", label = "profile", verbose_name = "User Profiles")]
pub struct ProfileConfig;
