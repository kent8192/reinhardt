//! # Database Integration Example
//!
//! This example demonstrates database integration with Reinhardt ORM.

pub mod apps;
pub mod config;
pub mod migrations;

// Re-export commonly used types
pub use apps::todos::models::Todo;
pub use apps::users::models::User;
pub use migrations::ExampleMigrations;
