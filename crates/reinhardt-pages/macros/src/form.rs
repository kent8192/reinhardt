//! Form macro entry point.
//!
//! This module implements the `form!` procedural macro for creating type-safe
//! forms with SSR/CSR support. The macro generates both server-side Form metadata
//! and client-side FormComponent with Signal bindings.
//!
//! ## Pipeline Architecture
//!
//! ```text
//! form! { ... }
//!     ↓ syn::parse2
//! FormMacro (untyped AST)
//!     ↓ validator::validate
//! TypedFormMacro (typed AST)
//!     ↓ codegen::generate
//! TokenStream (Rust code)
//! ```
//!
//! ## Example
//!
//! ```ignore
//! use reinhardt_pages::form;
//!
//! let login_form = form! {
//!     name: LoginForm,
//!     action: "/api/login",
//!     method: Post,
//!
//!     fields: {
//!         username: CharField {
//!             required,
//!             max_length: 150,
//!             label: "Username",
//!         },
//!         password: CharField {
//!             required,
//!             widget: PasswordInput,
//!             label: "Password",
//!         },
//!     },
//! };
//!
//! // Type-safe field access
//! let username = login_form.username();  // &Signal<String>
//!
//! // Convert to View
//! let view = login_form.into_view();
//! ```

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;

mod codegen;
mod validator;

/// Main entry point for the form! macro.
///
/// This function orchestrates the 3-stage pipeline:
/// 1. Parse: TokenStream → FormMacro (untyped AST)
/// 2. Validate: FormMacro → TypedFormMacro (typed AST)
/// 3. Generate: TypedFormMacro → TokenStream (Rust code)
pub(crate) fn form_impl(input: TokenStream) -> TokenStream {
	let input2 = TokenStream2::from(input);

	// Stage 1: Parse untyped AST
	let untyped_ast = match syn::parse2::<reinhardt_manouche::core::FormMacro>(input2) {
		Ok(ast) => ast,
		Err(e) => return e.to_compile_error().into(),
	};

	// Stage 2: Validate and transform to typed AST
	let typed_ast = match validator::validate(&untyped_ast) {
		Ok(ast) => ast,
		Err(e) => return e.to_compile_error().into(),
	};

	// Stage 3: Generate code
	codegen::generate(&typed_ast).into()
}
