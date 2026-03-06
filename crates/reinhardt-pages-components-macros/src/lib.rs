//! Procedural macros for reinhardt-pages-components
//!
//! This crate provides the `page!` and `form!` macros for declarative UI construction.

use proc_macro::TokenStream;

/// Placeholder for page! macro
///
/// # Examples
///
/// ```ignore
/// use reinhardt_pages_components::*;
///
/// page! {
///     Container {
///         children: [Alert { message: "Hello" }],
///     }
/// }
/// ```
#[proc_macro]
pub fn page(_input: TokenStream) -> TokenStream {
	todo!("page! macro implementation")
}

/// Placeholder for form! macro
///
/// # Examples
///
/// ```ignore
/// use reinhardt_pages_components::*;
///
/// form! {
///     LoginForm {
///         action: "/login",
///         remember_me: true,
///     }
/// }
/// ```
#[proc_macro]
pub fn form(_input: TokenStream) -> TokenStream {
	todo!("form! macro implementation")
}
