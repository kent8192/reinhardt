//! {{ app_name }} application module
//!
//! A Reinhardt Pages application with WASM frontend and server functions

pub mod admin;
pub mod models;
pub mod serializers;
pub mod urls;

#[cfg(target_arch = "wasm32")]
pub mod client;

#[cfg(not(target_arch = "wasm32"))]
pub mod server;

pub mod shared;

// Re-export commonly used types
pub use shared::types::*;
pub use shared::errors::*;

#[app_config(name = "{{ app_name }}", label = "{{ app_name }}")]
pub struct {{ camel_case_app_name }}Config;
