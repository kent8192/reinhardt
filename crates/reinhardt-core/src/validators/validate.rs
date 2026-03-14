//! Struct-level validation trait

use super::validation_errors::ValidationErrors;

/// Trait for struct-level validation.
///
/// Implementations collect per-field errors into [`ValidationErrors`].
/// Derive this trait with `#[derive(Validate)]` and `#[validate(...)]`
/// field attributes.
pub trait Validate {
	/// Validate all fields and return accumulated errors.
	fn validate(&self) -> Result<(), ValidationErrors>;
}
