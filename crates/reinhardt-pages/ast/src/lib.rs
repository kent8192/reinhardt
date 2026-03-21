//! AST definitions for the page! and form! macro DSLs.
//!
//! **DEPRECATED:** This crate is deprecated. Use `reinhardt-manouche` instead.
//!
//! This crate now re-exports types from `reinhardt-manouche` for backward compatibility.

#![deprecated(
	since = "0.1.0-alpha.2",
	note = "Use reinhardt-manouche instead. This crate will be removed in a future version."
)]

// Re-export everything from reinhardt-manouche for backward compatibility
pub use reinhardt_manouche::core::*;
pub use reinhardt_manouche::parser::*;

/// AST type definitions and enums.
pub mod types {
	pub use reinhardt_manouche::core::types::*;
}

/// Typed AST node representations.
pub mod typed_node {
	pub use reinhardt_manouche::core::typed_node::*;
}

/// Form-specific AST node types.
pub mod form_node {
	pub use reinhardt_manouche::core::form_node::*;
}

/// Typed form AST nodes with validation.
pub mod form_typed {
	pub use reinhardt_manouche::core::form_typed::*;
}
