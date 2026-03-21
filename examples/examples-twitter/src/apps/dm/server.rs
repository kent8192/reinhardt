//! DM server module
//!
//! Server-only components for direct messaging.

pub mod handlers;

pub use crate::apps::dm::shared::server_fn::*;
pub use handlers::DMHandler;
