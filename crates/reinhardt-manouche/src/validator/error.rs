//! Validation error types.

use proc_macro2::Span;

/// An error that occurred during validation.
#[derive(Debug)]
pub struct ValidationError {
	/// The span where the error occurred
	pub span: Span,
	/// The kind of error
	pub kind: ValidationErrorKind,
}

/// The kind of validation error.
#[derive(Debug)]
pub enum ValidationErrorKind {
	/// Unknown HTML element
	UnknownElement(String),
	/// Invalid attribute for element
	InvalidAttribute { element: String, attr: String },
	/// Missing required attribute
	MissingRequiredAttribute { element: String, attr: String },
	/// Duplicate attribute
	DuplicateAttribute(String),
	/// Invalid event handler
	InvalidEventHandler(String),
	/// Type mismatch
	TypeMismatch { expected: String, found: String },
	/// Invalid nesting (e.g., button inside button)
	InvalidNesting { parent: String, child: String },
	/// Void element cannot have children
	VoidElementWithChildren(String),
	/// Form-specific errors
	FormError(String),
}

impl ValidationError {
	/// Creates a new validation error.
	pub fn new(span: Span, kind: ValidationErrorKind) -> Self {
		Self { span, kind }
	}

	/// Converts to a `syn::Error` for macro error reporting.
	pub fn into_syn_error(self) -> syn::Error {
		let message = match self.kind {
			ValidationErrorKind::UnknownElement(e) => format!("unknown element: {}", e),
			ValidationErrorKind::InvalidAttribute { element, attr } => {
				format!("invalid attribute '{}' for element '{}'", attr, element)
			}
			ValidationErrorKind::MissingRequiredAttribute { element, attr } => {
				format!(
					"missing required attribute '{}' for element '{}'",
					attr, element
				)
			}
			ValidationErrorKind::DuplicateAttribute(attr) => {
				format!("duplicate attribute: {}", attr)
			}
			ValidationErrorKind::InvalidEventHandler(msg) => {
				format!("invalid event handler: {}", msg)
			}
			ValidationErrorKind::TypeMismatch { expected, found } => {
				format!("type mismatch: expected {}, found {}", expected, found)
			}
			ValidationErrorKind::InvalidNesting { parent, child } => {
				format!("invalid nesting: '{}' cannot be inside '{}'", child, parent)
			}
			ValidationErrorKind::VoidElementWithChildren(e) => {
				format!("void element '{}' cannot have children", e)
			}
			ValidationErrorKind::FormError(msg) => format!("form error: {}", msg),
		};
		syn::Error::new(self.span, message)
	}
}
