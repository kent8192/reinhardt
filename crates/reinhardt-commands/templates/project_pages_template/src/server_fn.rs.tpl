//! Server functions for {{ project_name }}.
//!
//! Server functions are callable from the WASM client. Each app under
//! `apps/` typically has a matching submodule here, e.g.:
//!
//! ```rust,ignore
//! pub mod polls;
//! ```
//!
//! Inside `server_fn/<app>.rs` declare functions with `#[server_fn]`:
//!
//! ```rust,ignore
//! use reinhardt::pages::server_fn::{ServerFnError, server_fn};
//!
//! #[server_fn]
//! pub async fn ping() -> std::result::Result<String, ServerFnError> {
//!     Ok("pong".to_string())
//! }
//! ```
//!
//! Apps will be added here by `reinhardt-admin startapp --with-pages`.
