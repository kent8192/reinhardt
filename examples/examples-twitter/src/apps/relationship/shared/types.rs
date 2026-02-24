//! Relationship shared types
//!
//! Types shared between client and server for user relationships.
//!
//! Note: Relationship operations use UserInfo from the auth app
//! since they operate on user data.

// Re-export UserInfo for convenience
// The actual UserInfo type is defined in auth::shared::types
pub use crate::apps::auth::shared::types::UserInfo;
