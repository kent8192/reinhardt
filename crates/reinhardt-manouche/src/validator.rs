//! Semantic validation and type transformation.
//!
//! This module transforms untyped AST into typed AST while performing
//! semantic validation and type checking.

pub mod error;
mod form;
mod head;
mod html_spec;
mod page;
mod style;

pub use error::*;
pub use form::{
	validate_form, validate_form_with_ambient_arguments_source, validate_redirect_on_success,
};
pub use head::validate_head;
pub use page::{classify_input_binding, validate_page, validate_page_accessibility};
pub use style::validate_style;

// Re-export typed AST types from core
pub use crate::core::TypedHeadMacro;
