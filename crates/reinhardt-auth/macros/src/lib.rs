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

	match guard_parser::parse_guard_expr(&input_str) {
		Ok(expr) => {
			let output = guard_codegen::generate_guard_type(&expr);
			output.into()
		}
		Err(err) => {
			let msg = format!("guard!() parse error: {err}");
			let output = quote::quote! { compile_error!(#msg) };
			output.into()
		}
	}
}
