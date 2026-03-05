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

pub mod create;
pub mod dashboard;
pub mod delete;
pub mod detail;
pub mod error;
pub mod export;
pub mod fields;
pub mod import;
pub mod limits;
pub mod list;
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
