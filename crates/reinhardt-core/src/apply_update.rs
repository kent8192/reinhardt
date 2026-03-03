//! Trait for applying partial updates to target structs.
//!
//! The [`ApplyUpdate`] trait provides a standardized way to apply partial updates
//! from an update request struct to a target model struct. This eliminates the
//! repetitive `if let Some(v) = self.field { target.field = v; }` boilerplate
//! commonly found in PATCH endpoint handlers.
//!
//! # Usage
//!
//! Use the `#[apply_update(target(TargetType))]` attribute macro to automatically
//! derive the implementation:
//!
//! ```rust,ignore
//! #[apply_update(target(User))]
//! struct UpdateUserRequest {
//!     pub name: Option<String>,
//!     pub email: Option<String>,
//!     #[apply_update(skip)]
//!     pub _metadata: Option<String>,
//! }
//! ```

/// Trait for applying partial updates from one struct to another.
///
/// `Option<T>` fields apply only when `Some`, while non-`Option` fields
/// are always applied.
pub trait ApplyUpdate<T> {
	/// Consumes `self` and applies updates to the given target.
	fn apply_to(self, target: &mut T);
}
