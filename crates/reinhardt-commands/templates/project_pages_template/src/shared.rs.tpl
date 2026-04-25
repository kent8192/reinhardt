//! Types and utilities shared between the WASM client and the server.
//!
//! - `forms` (server-only) — `Form` definitions used to generate
//!   `FormMetadata` (e.g. CSRF tokens) for the WASM client.
//! - `types` — DTOs serialized over server functions.

#[cfg(server)]
pub mod forms;
pub mod types;
