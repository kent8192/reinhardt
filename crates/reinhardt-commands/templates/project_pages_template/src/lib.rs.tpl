//! {{ project_name }} library
//!
//! Top-level crate for {{ project_name }}. The module layout follows the
//! Reinhardt basics tutorial:
//! - `apps`         — application code (each app has server-side routes and client-side pages)
//! - `client`       — WASM-only frontend (mounted by `bin/manage.rs`)
//! - `config`       — project configuration (settings, urls, apps, wasm)

// Application modules
pub mod apps;
pub mod config;

// Client-only modules (WASM)
#[cfg(client)]
pub mod client;

// Re-export commonly used items
#[cfg(server)]
pub use config::settings::get_settings;
#[cfg(server)]
pub use config::urls::routes;
