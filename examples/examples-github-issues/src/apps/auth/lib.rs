//! auth application module
//!
//! A RESTful API application

use reinhardt::app_config;

pub mod admin;
pub mod models;
pub mod serializers;
#[cfg(test)]
pub mod tests;
pub mod urls;
pub mod views;

#[app_config(name = "auth", label = "auth")]
pub struct AuthConfig;

// Re-export as Auth for use in src/apps.rs
pub use AuthConfig as Auth;
