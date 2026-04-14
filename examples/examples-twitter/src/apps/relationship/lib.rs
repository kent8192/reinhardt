//! relationship application module
//!
//! User relationship models for examples-twitter

#[cfg(native)]
use reinhardt::app_config;

pub mod shared;
pub mod urls;

#[cfg(wasm)]
pub mod client;

#[cfg(native)]
pub mod server;

#[cfg(test)]
pub mod tests;

#[cfg(native)]
#[app_config(
	name = "relationship",
	label = "relationship",
	verbose_name = "User Relationships"
)]
pub struct RelationshipConfig;
