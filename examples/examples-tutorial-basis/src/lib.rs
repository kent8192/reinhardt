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

// Server-side modules
#[cfg(server)]
pub mod apps;
#[cfg(server)]
pub mod config;

// Client-side modules
#[cfg(client)]
pub mod client;

// Server function definitions (both WASM and server)
pub mod server_fn;

// Shared types (both WASM and server)
pub mod shared;
