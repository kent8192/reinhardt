//! examples-twitter library
//!
//! This is a full-stack Twitter clone built with reinhardt-pages.
//! - Server-side: REST API with server functions
//! - Client-side: WASM frontend with reactive UI

// ============================================================================
// Server-only modules (non-WASM)
// ============================================================================
#[cfg(server)]
mod server_only {
	// Re-export internal crates for macro-generated code
	pub use reinhardt::core::async_trait;
	pub use reinhardt::reinhardt_apps;
	pub use reinhardt::reinhardt_core;
	pub use reinhardt::reinhardt_di::params;
	pub use reinhardt::reinhardt_http;
}
#[cfg(server)]
pub use server_only::*;

// ============================================================================
// Applications (shared between WASM and server with conditional modules)
// ============================================================================
pub mod apps;

// ============================================================================
// Server-only modules
// ============================================================================
#[cfg(server)]
pub mod config;
#[cfg(server)]
pub mod migrations;

// ============================================================================
// Client-only modules (WASM)
// ============================================================================
#[cfg(client)]
pub mod core;

// ============================================================================
// Re-exports for convenience
// ============================================================================
#[cfg(server)]
pub use config::settings::get_settings;

// Test utilities (available for testing on server)
#[cfg(server)]
pub mod test_utils;
