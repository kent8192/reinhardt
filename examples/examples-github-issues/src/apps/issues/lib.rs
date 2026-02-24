//! issues application module
//!
//! A RESTful API application

use reinhardt::app_config;

pub mod admin;
pub mod errors;
pub mod models;
pub mod serializers;
#[cfg(test)]
pub mod tests;
pub mod urls;
pub mod views;

#[app_config(name = "issues", label = "issues")]
pub struct IssuesConfig;

// Re-export as Issues for use in src/apps.rs
pub use IssuesConfig as Issues;
