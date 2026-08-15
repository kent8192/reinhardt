//! Procedural macros for `reinhardt-auth`.
//!
//! Provides the [`guard!`] macro for concise permission guard type expressions.
//!
//! # Usage
//!
//! ```rust,ignore
//! use reinhardt_auth::guard;
//!
//! #[get("/admin/")]
//! pub async fn admin_view(
//!     #[inject] _: guard!(IsAdminUser & IsActiveUser),
//! ) -> ViewResult<Response> {
//!     // Only active admin users reach here
//! }
//! ```
//!
//! # Supported Syntax
//!
//! | Syntax | Meaning |
//! |--------|---------|
//! | `A` | Single permission type |
//! | `A & B` | AND: both must pass |
//! | `A \| B` | OR: at least one must pass |
//! | `!A` | NOT: inverts the check |
//! | `(A \| B) & C` | Parenthesized grouping |
//! | `mod::Type` | Qualified type paths |

#![warn(missing_docs)]

mod guard_codegen;
mod guard_parser;

use proc_macro::TokenStream;

/// Generates a permission guard type from a concise expression.
///
/// The macro outputs a TYPE (not a value), designed for use with `#[inject]`:
///
/// ```rust,ignore
/// #[inject] _: guard!(IsAdminUser & IsActiveUser)
/// // expands to:
/// // #[inject] _: reinhardt_auth::guard::Guard<reinhardt_auth::guard::All<(IsAdminUser, IsActiveUser)>>
/// ```
///
/// # Operators
///
/// - `&` — AND combinator (`All`)
/// - `|` — OR combinator (`Any`)
/// - `!` — NOT combinator (`Not`)
/// - `()` — grouping for precedence override
///
/// Precedence: `!` > `&` > `|`
///
/// # Examples
///
/// ```rust,ignore
/// use reinhardt_auth::guard;
///
/// // Single permission
/// type G1 = guard!(IsAdminUser);
///
/// // AND
/// type G2 = guard!(IsAdminUser & IsActiveUser);
///
/// // OR
/// type G3 = guard!(IsAdminUser | IsActiveUser);
///
/// // NOT
/// type G4 = guard!(!IsAdminUser);
///
/// // Complex
/// type G5 = guard!((IsAdminUser | IsActiveUser) & !IsAuthenticated);
/// ```
#[proc_macro]
pub fn guard(input: TokenStream) -> TokenStream {
	let input_str = input.to_string();
	guard_impl(&input_str).into()
}

fn guard_impl(input: &str) -> proc_macro2::TokenStream {
	match guard_parser::parse_guard_expr(input) {
		Ok(expr) => {
			let output = guard_codegen::generate_guard_type(&expr);
			output
		}
		Err(err) => {
			let msg = format!("guard!() parse error: {err}");
			let output = quote::quote! { compile_error!(#msg) };
			output
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn guard_accepts_expression() {
		let output = guard_impl("IsAuthenticated & IsAdminUser");
		let expected = quote::quote! {
			reinhardt_auth::guard::Guard<
				reinhardt_auth::guard::All<(IsAuthenticated, IsAdminUser)>
			>
		};

		assert_eq!(output.to_string(), expected.to_string());
	}

	#[test]
	fn guard_reports_invalid_syntax() {
		let output = guard_impl("@");
		let expected = quote::quote! {
			compile_error!("guard!() parse error: failed to parse guard expression: `@`")
		};

		assert_eq!(output.to_string(), expected.to_string());
	}

	#[test]
	fn guard_reports_unsupported_escaped_permission() {
		let output = guard_impl(r#"HasPerm("blog\"edit")"#);
		let expected = quote::quote! {
			reinhardt_auth::guard::Guard<compile_error!(
				"HasPerm(\"...\") is not yet supported in guard!() macro. \
					 Define a custom Permission type instead."
			)>
		};

		assert_eq!(output.to_string(), expected.to_string());
	}
}
