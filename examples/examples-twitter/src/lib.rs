//! examples-twitter library
//!
//! This is a full-stack Twitter clone built with reinhardt-pages.
//! - Server-side: REST API with server functions
//! - Client-side: WASM frontend with reactive UI

// ============================================================================
// Server-only modules (non-WASM)
// ============================================================================
#[cfg(native)]
mod server_only {
	// Re-export internal crates for macro-generated code
	pub use reinhardt::core::async_trait;
	pub use reinhardt::reinhardt_apps;
	pub use reinhardt::reinhardt_core;
	pub use reinhardt::reinhardt_di::params;
	pub use reinhardt::reinhardt_http;
}
#[cfg(native)]
pub use server_only::*;

// ============================================================================
// Applications (shared between WASM and server with conditional modules)
// ============================================================================
pub mod apps;

// ============================================================================
// Server-only modules
// ============================================================================
pub mod config;
#[cfg(native)]
pub mod migrations;

// ============================================================================
// Client-only modules (WASM)
// ============================================================================
#[cfg(wasm)]
pub mod core;

// ============================================================================
// Re-exports for convenience
// ============================================================================
#[cfg(native)]
pub use config::settings::get_settings;

// Test utilities (available for testing on server)
#[cfg(native)]
pub mod test_utils;
