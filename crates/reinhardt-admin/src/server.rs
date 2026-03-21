//! Server Functions for Reinhardt admin panel
//!
//! This crate provides Server Functions that handle admin panel operations,
//! replacing the traditional REST API handlers with reinhardt-pages Server Functions.
//!
//! # Architecture
//!
//! Each module contains Server Functions for specific admin operations:
//! - `dashboard` - Dashboard data retrieval
//! - `list` - List view operations
//! - `detail` - Detail view operations
//! - `create` - Create operations
//! - `update` - Update operations
//! - `delete` - Delete operations (including bulk delete)
//! - `export` - Export operations
//! - `import` - Import operations
//!
//! # Server Functions
//!
//! Server Functions use `#[server_fn]` macro and support:
//! - Automatic DI injection via `#[inject]` parameter
//! - JSON codec for complex request/response types
//! - Automatic error conversion to `ServerFnError`
//! - CSRF protection (handled automatically by reinhardt-pages)
//!
//! # Example
//!
//! ```ignore
//! use reinhardt_admin::server::dashboard::get_dashboard;
//!
//! // In your app
//! let dashboard_data = get_dashboard().await?;
//! ```

// The `#[server_fn]` proc macro generates internal modules that cannot have doc comments.
// Allow missing docs for all server function submodules.
#[allow(missing_docs, deprecated)] // CurrentUser deprecated, will migrate to AuthUser in 0.2.0
pub mod create;
#[allow(missing_docs)]
pub mod dashboard;
#[allow(missing_docs)]
pub mod delete;
#[allow(missing_docs)]
pub mod detail;
/// Error handling utilities for server functions.
pub mod error;
#[allow(missing_docs)]
pub mod export;
#[allow(missing_docs)]
pub mod fields;
#[allow(missing_docs)]
pub mod import;
/// Request size and rate limits for server functions.
pub mod limits;
#[allow(missing_docs, deprecated)] // CurrentUser deprecated, will migrate to AuthUser in 0.2.0
pub mod list;
#[allow(missing_docs)]
pub mod update;

pub mod audit;
pub mod security;

// Server-side only modules
#[cfg(not(target_arch = "wasm32"))]
pub mod type_inference;
#[cfg(not(target_arch = "wasm32"))]
pub mod validation;

// Re-exports
pub use create::*;
pub use dashboard::*;
pub use delete::*;
pub use detail::*;
pub use export::*;
pub use fields::*;
pub use import::*;
pub use list::*;
pub use update::*;
