//! {{ app_name }} application module
//!
//! A Reinhardt Pages application with WASM frontend and server functions

use reinhardt::app_config;

// Server-side modules
pub mod admin;
pub mod models;
pub mod serializers;
pub mod urls;
pub mod views;

// Client-side modules
#[cfg(client)]
pub mod client;

// Server-side modules
#[cfg(server)]
pub mod server;

// Shared types (both WASM and server)
pub mod shared;

// Re-export commonly used types
pub use shared::errors::*;
pub use shared::types::*;

#[app_config(name = "{{ app_name }}", label = "{{ app_name }}")]
pub struct {{ camel_case_app_name }}Config;
