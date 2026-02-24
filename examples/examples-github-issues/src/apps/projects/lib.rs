//! projects application module
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

#[app_config(name = "projects", label = "projects")]
pub struct ProjectsConfig;

// Re-export as Projects for use in src/apps.rs
pub use ProjectsConfig as Projects;
