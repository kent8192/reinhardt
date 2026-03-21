//! Secret management infrastructure.
//!
//! Provides secret storage, retrieval, rotation, and audit capabilities
//! through pluggable provider backends.

/// Audit logging for secret access operations.
pub mod audit;
/// Secret provider backend implementations.
pub mod providers;
/// Automatic secret rotation support.
pub mod rotation;
/// Core secret types, errors, and traits.
pub mod types;

pub use types::*;
