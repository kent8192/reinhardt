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
//! - `inline_edit` - Atomic changelist inline edits
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
#[allow(missing_docs)]
pub mod action;
#[cfg(server)]
pub(crate) mod admin_auth;
#[allow(missing_docs)]
pub mod create;
#[allow(missing_docs)]
pub mod dashboard;
#[allow(missing_docs)]
pub mod delete;
#[allow(missing_docs)]
pub mod detail;
/// Error handling utilities for server functions.
#[cfg(server)]
pub mod error;
#[allow(missing_docs)]
pub mod export;
#[allow(missing_docs)]
pub mod fields;
#[allow(missing_docs)]
pub mod import;
pub(crate) mod inline;
// The server_fn macro generates undocumented marker items inside this module.
#[allow(missing_docs)]
pub mod inline_edit;
/// Request size and rate limits for server functions.
pub mod limits;
#[allow(missing_docs)]
pub mod list;
#[allow(missing_docs)]
pub mod login;
#[allow(missing_docs)]
pub mod logout;
mod serde_helpers;
#[allow(missing_docs)]
pub mod update;
#[cfg(server)]
pub(crate) mod user;

#[allow(missing_docs)]
pub mod audit;
/// Cookie-based JWT authentication middleware for admin panel.
#[cfg(not(target_arch = "wasm32"))]
pub mod cookie_auth;
/// Origin guard middleware restricting admin server functions to SPA-only access.
#[cfg(not(target_arch = "wasm32"))]
pub mod origin_guard;
pub mod security;

// Server-side only modules
#[cfg(server)]
pub mod type_inference;
#[cfg(server)]
pub mod validation;

// Re-exports
pub use action::*;
#[cfg(server)]
pub use admin_auth::AdminAuthenticatedUser;
pub use audit::get_history;
pub use create::*;
pub use dashboard::*;
pub use delete::*;
pub use detail::*;
pub use export::*;
pub use fields::*;
pub use import::*;
pub use inline_edit::*;
pub use list::*;
pub use update::*;
#[cfg(server)]
pub use user::AdminDefaultUser;
