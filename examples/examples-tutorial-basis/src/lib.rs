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

// Applications (declared on both targets; submodules cfg-gate themselves)
pub mod apps;

// Configuration (urls unconditional, rest server-only)
pub mod config;

// App-level page entry points live under `apps::<app>::pages`; executable
// browser modules inside `client` remain cfg-gated.
pub mod client;

// Shared modules (both WASM and server)
//
// Server functions are now scoped under each app — they live alongside
// the app's models / views / urls in `apps::<app>::server_fn`, which
// keeps related code together and removes the top-level `server_fn`
// module that previously had to mirror the app list.
pub mod shared;

// Re-exports
#[cfg(server)]
pub use config::settings::get_settings;
