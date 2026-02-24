//! dm application module
//!
//! Direct message models for examples-twitter

use reinhardt::app_config;

pub mod admin;
pub mod models;
pub mod shared;
pub mod urls;

#[cfg(client)]
pub mod client;

#[cfg(server)]
pub mod server;

#[cfg(test)]
pub mod tests;

#[app_config(name = "dm", label = "dm", verbose_name = "Direct Messages")]
pub struct DmConfig;
