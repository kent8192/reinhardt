//! auth application module
//!
//! User authentication models for examples-twitter

#[cfg(native)]
use reinhardt::app_config;

#[cfg(native)]
pub mod models;
pub mod shared;
pub mod urls;

#[cfg(wasm)]
pub mod client;

#[cfg(native)]
pub mod server;

#[cfg(test)]
pub mod tests;

#[cfg(native)]
#[app_config(name = "auth", label = "auth", verbose_name = "Authentication")]
pub struct AuthConfig;
