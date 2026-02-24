//! User model for authentication
//!
//! Defines the User model using reinhardt's DefaultUser implementation.

use reinhardt::DefaultUser;

/// User type for this application
/// Re-exports reinhardt's DefaultUser which includes:
/// - BaseUser trait implementation
/// - FullUser trait implementation
/// - All necessary fields (id, username, email, password_hash, etc.)
pub type User = DefaultUser;
