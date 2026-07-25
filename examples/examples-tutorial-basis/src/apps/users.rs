//! Users application
//!
//! Provides session-based authentication for the tutorial-basis example.
//! Defines a minimal `User` model and exposes server functions for login,
//! logout, sign-up, and current-user introspection via
//! `crate::apps::users::server_fn`.

#[cfg(server)]
use reinhardt::app_config;

#[cfg(client)]
pub mod client;
pub mod models;
#[cfg(server)]
pub mod server;
pub mod server_fn;
#[cfg(server)]
pub mod services;
pub mod urls;

#[cfg(server)]
#[app_config(name = "users", label = "users")]
pub struct UsersConfig;
