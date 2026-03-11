//! Auth server module
//!
//! Re-exports server functions from the shared module for backward compatibility.
//! The actual implementations live in `shared::server_fn` to ensure WASM compatibility.

pub use crate::apps::auth::shared::server_fn;
