//! Procedural macros for compile-time URL path validation in Reinhardt.
//!
//! This crate provides macros that validate URL paths at compile time,
//! ensuring that routing paths follow correct syntax before runtime.
//!
//! # Examples
//!
//! ```
//! use reinhardt_routers_macros::path;
//!
//! // Valid paths
//! let pattern = path!("/users/");
//! let pattern = path!("/users/{id}/");
//! let pattern = path!("/users/{user_id}/posts/{post_id}/");
//! ```
//!
//! # Compile-time Validation
//!
//! The following will fail to compile:
//!
//! ```compile_fail
//! use reinhardt_routers_macros::path;
//!
//! // Error: path must start with '/'
//! let pattern = path!("users/");
//! ```
//!
//! ```compile_fail
//! use reinhardt_routers_macros::path;
//!
//! // Error: unmatched '{'
//! let pattern = path!("/users/{id/");
//! ```

use proc_macro::TokenStream;
use quote::quote;
use syn::{LitStr, parse_macro_input};

mod validation;

use validation::{PathValidationError, validate_path_syntax};

/// Validates URL path syntax at compile time.
///
/// This macro ensures that URL paths follow the correct format:
/// - Must start with `/`
/// - Parameters must be enclosed in `{}`
/// - Parameter names must be valid snake_case identifiers
/// - No double slashes (except after protocol)
///
/// # Examples
///
/// ```
/// use reinhardt_routers_macros::path;
///
/// let simple = path!("/users/");
/// let with_param = path!("/users/{id}/");
/// let multiple_params = path!("/users/{user_id}/posts/{post_id}/");
/// ```
///
/// # Errors
///
/// This macro will produce compile-time errors for invalid paths:
///
/// ```compile_fail
/// use reinhardt_routers_macros::path;
///
// Error: path must start with '/'
/// let invalid = path!("users/");
/// ```
///
/// ```compile_fail
/// use reinhardt_routers_macros::path;
///
// Error: parameter names must be snake_case
/// let invalid = path!("/users/{userId}/");
/// ```
#[proc_macro]
pub fn path(input: TokenStream) -> TokenStream {
	let path_str = parse_macro_input!(input as LitStr);
	let path = path_str.value();

	// Validate path syntax
	if let Err(e) = validate_path_syntax(&path) {
		let error_msg = format_error_message(&e);
		return syn::Error::new(path_str.span(), error_msg)
			.to_compile_error()
			.into();
	}

	// Return the validated path string as-is
	// The consuming code is expected to handle PathPattern creation
	quote! {
		#path_str
	}
	.into()
}

/// Formats validation errors with helpful context
fn format_error_message(error: &PathValidationError) -> String {
	match error {
		PathValidationError::MustStartWithSlash => "URL path must start with '/'\n\
             Example: path!(\"/users/\") instead of path!(\"users/\")"
			.to_string(),
		PathValidationError::UnmatchedOpenBrace(pos) => {
			format!(
				"Unmatched '{{' at position {}\n\
                 All parameter markers must be properly closed with '}}'",
				pos
			)
		}
		PathValidationError::UnmatchedCloseBrace(pos) => {
			format!(
				"Unmatched '}}' at position {}\n\
                 Found closing brace without matching opening brace",
				pos
			)
		}
		PathValidationError::EmptyParameterName(pos) => {
			format!(
				"Empty parameter name at position {}\n\
                 Parameter names must not be empty: use {{param_name}}",
				pos
			)
		}
		PathValidationError::InvalidParameterName { name, position } => {
			format!(
				"Invalid parameter name '{}' at position {}\n\
                 Parameter names must be valid snake_case identifiers:\n\
                 - Start with lowercase letter or underscore\n\
                 - Contain only lowercase letters, digits, and underscores\n\
                 Example: {{user_id}} instead of {{userId}} or {{user-id}}",
				name, position
			)
		}
		PathValidationError::DoubleSlash(pos) => {
			format!(
				"Double slash '//' found at position {}\n\
                 Paths should not contain consecutive slashes",
				pos
			)
		}
		PathValidationError::InvalidCharacter { ch, position } => {
			format!(
				"Invalid character '{}' at position {}\n\
                 URL paths may only contain:\n\
                 - Alphanumeric characters (a-z, A-Z, 0-9)\n\
                 - Hyphens (-)\n\
                 - Underscores (_)\n\
                 - Slashes (/)\n\
                 - Dots (.)\n\
                 - Wildcards (*)\n\
                 - Curly braces for parameters ({{, }})",
				ch, position
			)
		}
		PathValidationError::NestedParameters(pos) => {
			format!(
				"Nested parameter at position {}\n\
                 Parameters cannot be nested: use {{outer}} instead of {{{{inner}}}}",
				pos
			)
		}
		PathValidationError::ConsecutiveParameters(pos) => {
			format!(
				"Consecutive parameters without separator at position {}\n\
                 Parameters must be separated by a static segment (e.g., '/')\n\
                 Example: /{{id}}/{{name}}/ instead of /{{id}}{{name}}/",
				pos
			)
		}
		PathValidationError::WildcardNotAtEnd(pos) => {
			format!(
				"Wildcard '*' at position {} must appear only in the last path segment\n\
                 Wildcards can only be used at the end of a path\n\
                 Example: /static/* instead of /*/files",
				pos
			)
		}
		PathValidationError::PathTraversal(pos) => {
			format!(
				"Path traversal sequence '..' detected at position {}\n\
                 URL paths must not contain '..' to prevent directory traversal attacks",
				pos
			)
		}
		PathValidationError::DuplicateParameterName { name, position } => {
			format!(
				"Duplicate parameter name '{}' at position {}\n\
                 Each parameter name must be unique within the path",
				name, position
			)
		}
	}
}
