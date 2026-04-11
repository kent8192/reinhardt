//! Procedural macros for Reinhardt dependency injection
//!
//! This crate provides FastAPI-style dependency injection macros:
//! - `#[injectable]` - Mark a struct as injectable with automatic registration
//! - `#[injectable_factory]` - Mark an async function as a dependency factory

use proc_macro::TokenStream;
use syn::{DeriveInput, ItemFn, parse_macro_input};

mod crate_paths;
mod injectable;
mod injectable_factory;
mod utils;

/// Mark a struct as injectable and register it with the global registry
///
/// This macro automatically derives `Clone` for the annotated type if it is
/// not already derived. `Clone` is used by `into_inner()` and `injectable_factory` patterns.
///
/// # Attribute Ordering
///
/// **`#[injectable]` must be placed above `#[derive(...)]` attributes.**
///
/// In Rust 2024 edition, attribute macros can only see attributes listed
/// below them. If `#[derive(Clone)]` appears above `#[injectable]`, the
/// macro cannot detect it and will add a duplicate `#[derive(Clone)]`,
/// causing a compilation error.
///
/// ```ignore
/// // Correct — #[injectable] is the outermost attribute
/// #[injectable]
/// #[derive(Default, Debug)]
/// struct MyService {
///     #[no_inject]
///     name: String,
/// }
///
/// // Incorrect — #[derive] above #[injectable] is not visible to the macro
/// #[derive(Default, Debug)]
/// #[injectable]  // Cannot detect derives above; may cause duplicate Clone
/// struct MyService {
///     #[no_inject]
///     name: String,
/// }
/// ```
///
/// # Example
///
/// ```ignore
/// use reinhardt_di_macros::injectable;
///
/// #[injectable]
/// #[scope(singleton)]
/// struct Config {
///     #[no_inject]
///     database_url: String,
/// }
/// ```
///
/// # Attributes
///
/// - `#[scope(singleton)]` - Singleton scope (default)
/// - `#[scope(request)]` - Request scope
/// - `#[scope(transient)]` - Transient scope
#[proc_macro_attribute]
pub fn injectable(args: TokenStream, input: TokenStream) -> TokenStream {
	let input = parse_macro_input!(input as DeriveInput);

	injectable::injectable_impl(args.into(), input)
		.unwrap_or_else(|e| e.to_compile_error())
		.into()
}

/// Mark an async function as a dependency factory
///
/// # Example
///
/// ```ignore
/// use reinhardt_di::Depends;
/// use reinhardt_di_macros::injectable_factory;
///
/// #[injectable_factory]
/// #[scope(singleton)]
/// async fn create_database(#[inject] config: Depends<Config>) -> DatabaseConnection {
///     DatabaseConnection::connect(&config.database_url).await.unwrap()
/// }
/// ```
///
/// # Attributes
///
/// - `#[scope(singleton)]` - Singleton scope (default)
/// - `#[scope(request)]` - Request scope
/// - `#[scope(transient)]` - Transient scope
/// - `#[inject]` - Mark function parameters for automatic injection
#[proc_macro_attribute]
pub fn injectable_factory(args: TokenStream, input: TokenStream) -> TokenStream {
	let input = parse_macro_input!(input as ItemFn);

	injectable_factory::injectable_factory_impl(args.into(), input)
		.unwrap_or_else(|e| e.to_compile_error())
		.into()
}
