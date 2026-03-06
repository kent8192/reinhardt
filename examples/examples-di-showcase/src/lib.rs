//! examples-di-showcase library
//!
//! Demonstrates Reinhardt's FastAPI-style dependency injection system,
//! including custom `Injectable` types, nested dependencies, cache control,
//! and singleton scope.

pub mod apps;
pub mod config;

// Re-export commonly used items
pub use config::settings::get_settings;
pub use config::urls::routes;
