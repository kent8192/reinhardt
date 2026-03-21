//! Auth shared types module
//!
//! Contains types shared between client and server for authentication.
//! These types are serializable and can be sent over the wire.

pub mod server_fn;
pub mod types;

pub use types::*;
