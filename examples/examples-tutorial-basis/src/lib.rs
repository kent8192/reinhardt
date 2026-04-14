//! Reinhardt Basis Tutorial Example - Polling Application with Pages
//!
//! This example demonstrates the concepts covered in the Reinhardt basis tutorial:
//! - Project setup and configuration
//! - Database models and ORM
//! - Views with reinhardt-pages (WASM + SSR)
//! - Forms and generic views
//! - Testing
//! - Static files
//! - Admin panel customization

// Server-only re-exports for macro-generated code
#[cfg(native)]
mod server_only {
	pub use reinhardt::core::async_trait;
	pub use reinhardt::reinhardt_apps;
	pub use reinhardt::reinhardt_core;
	pub use reinhardt::reinhardt_di::params;
	pub use reinhardt::reinhardt_http;
}
#[cfg(native)]
pub use server_only::*;

// Applications (server-only, polls uses ServerRouter)
#[cfg(native)]
pub mod apps;

// Configuration (urls unconditional, rest server-only)
pub mod config;

// Client-only modules (WASM)
#[cfg(wasm)]
pub mod client;

// Shared modules (both WASM and server)
pub mod server_fn;
pub mod shared;

// Re-exports
#[cfg(native)]
pub use config::settings::get_settings;
