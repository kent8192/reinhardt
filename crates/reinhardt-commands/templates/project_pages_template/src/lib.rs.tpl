//! {{ project_name }} library
//!
//! Top-level crate for {{ project_name }}. The module layout follows the
//! Reinhardt basics tutorial:
//! - `apps`         — server-side application code (uses ServerRouter)
//! - `client`       — WASM-only frontend (mounted by `bin/manage.rs`)
//! - `config`       — project configuration (settings, urls, apps, wasm)
//! - `server_fn`    — server functions callable from the WASM client
//! - `shared`       — types shared between WASM and server

// Server-only re-exports for macro-generated code.
//
// Server-side macros (`#[routes]`, `#[server_fn]`, etc.) reference framework
// crates by their internal paths (`reinhardt_apps`, `reinhardt_core`, ...).
// Re-export them under `crate::*` so the generated code resolves regardless
// of feature combination.
#[cfg(server)]
mod server_only {
	pub use reinhardt::core::async_trait;
	pub use reinhardt::reinhardt_apps;
	pub use reinhardt::reinhardt_core;
	pub use reinhardt::reinhardt_di::params;
	pub use reinhardt::reinhardt_http;
}
#[cfg(server)]
pub use server_only::*;

// Server-side modules
#[cfg(server)]
pub mod apps;
pub mod config;

// Client-only modules (WASM)
#[cfg(client)]
pub mod client;

// Modules shared between WASM and server
pub mod server_fn;
pub mod shared;

// Re-export commonly used items
#[cfg(server)]
pub use config::settings::get_settings;
#[cfg(server)]
pub use config::urls::routes;
