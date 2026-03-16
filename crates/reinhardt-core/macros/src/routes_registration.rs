//! Routes attribute macro implementation
//!
//! This module implements the `#[routes]` attribute macro that allows
//! functions to be registered as URL pattern providers for automatic
//! discovery by the framework.
//!
//! # Important: Single Usage Only
//!
//! **Only one function per project can be annotated with `#[routes]`.**
//! If multiple `#[routes]` attributes are used, the linker will fail with a
//! "duplicate symbol" error for `__reinhardt_routes_registration_marker`.
//!
//! To organize routes across multiple files, use the `.mount()` method:
//!
//! ```rust,ignore
//! // Only ONE function in the project should have #[routes]
//! #[routes]
//! pub fn routes() -> UnifiedRouter {
//!     UnifiedRouter::new()
//!         .mount("/api/", api::routes())   // api::routes() is NOT annotated with #[routes]
//!         .mount("/admin/", admin::routes())
//!         .client(|c| c.route("/", home_page))
//! }
//! ```
//!
//! # Macro Syntax
//!
//! ```rust,ignore
//! #[routes]
//! pub fn routes() -> UnifiedRouter {
//!     UnifiedRouter::new()
//!         .server(|s| s.endpoint(views::index))
//!         .client(|c| c.route("/", home_page))
//! }
//! ```
//!
//! # Generated Code
//!
//! The macro preserves the original function and adds `inventory::submit!`
//! registration code. The generated code is feature-independent to avoid
//! feature context mismatches between the library and downstream crates:
//!
//! ```rust,ignore
//! // Input:
//! #[routes]
//! pub fn routes() -> UnifiedRouter {
//!     UnifiedRouter::new()
//! }
//!
//! // Generated output:
//! pub fn routes() -> UnifiedRouter {
//!     UnifiedRouter::new()
//! }
//!
//! const _: () = {
//!     fn __get_server_router() -> ::std::sync::Arc<::reinhardt::ServerRouter> {
//!         let unified = routes();
//!         ::std::sync::Arc::new(unified.into_server())
//!     }
//!
//!     ::reinhardt::inventory::submit! {
//!         ::reinhardt::UrlPatternsRegistration::__macro_new(__get_server_router)
//!     }
//! };
//! ```

use crate::crate_paths::get_reinhardt_crate;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{ItemFn, Result};

/// Implementation of the `#[routes]` attribute macro
///
/// This function generates code that:
/// 1. Preserves the original function definition
/// 2. Adds `inventory::submit!` to register the function with the framework
///
/// The macro generates feature-independent code that only registers the
/// server router. The client router is handled within the library's own
/// feature-gated code via `with_client_router()`, avoiding the problem
/// where `#[cfg(feature = "client-router")]` in macro output would be
/// evaluated in the downstream crate's feature context.
///
/// # Parameters
///
/// * `_args` - Attribute arguments (currently unused, reserved for future use)
/// * `input` - The function to annotate
///
/// # Returns
///
/// Generated code as a `TokenStream`
///
/// # Errors
///
/// Returns an error if the function signature is invalid (e.g., missing return type)
pub(crate) fn routes_impl(_args: TokenStream, input: ItemFn) -> Result<TokenStream> {
	let reinhardt = get_reinhardt_crate();

	let fn_name = &input.sig.ident;
	let fn_vis = &input.vis;
	let fn_attrs = &input.attrs;
	let fn_sig = &input.sig;
	let fn_block = &input.block;

	// Validate that the function has a return type
	if matches!(input.sig.output, syn::ReturnType::Default) {
		return Err(syn::Error::new_spanned(
			&input.sig,
			"#[routes] function must have a return type (-> UnifiedRouter)",
		));
	}

	// Generate the original function, the inventory registration, and the linker marker.
	// The generated code is intentionally feature-independent: only the server router
	// is registered here. The client router is set via with_client_router() in library
	// code that has its own proper #[cfg(feature = "client-router")] gates.
	// Note: Rust 2024 edition requires unsafe for #[no_mangle] and #[link_section] attributes.
	// The inventory::submit! macro uses #[link_section] internally.
	let expanded = quote! {
		#(#fn_attrs)*
		#fn_vis #fn_sig #fn_block

		// Allow unsafe attributes used by inventory::submit! (#[link_section])
		// Required for Rust 2024 edition compatibility
		#[allow(unsafe_attr_outside_unsafe)]
		const _: () = {
			// Server router extraction function
			fn __get_server_router() -> ::std::sync::Arc<#reinhardt::ServerRouter> {
				let unified = #fn_name();
				::std::sync::Arc::new(unified.into_server())
			}

			// Register with inventory using feature-independent internal constructor
			#reinhardt::inventory::submit! {
				#reinhardt::UrlPatternsRegistration::__macro_new(__get_server_router)
			}
		};

		// Linker marker to enforce single #[routes] usage.
		// If multiple #[routes] macros exist, the linker will fail with a
		// "duplicate symbol" error for `__reinhardt_routes_registration_marker`.
		//
		// This provides compile-time (link-time) enforcement that only one
		// #[routes] function can exist in the entire project.
		#[doc(hidden)]
		#[unsafe(no_mangle)]
		#[allow(non_upper_case_globals, dead_code)]
		// non_upper_case_globals: Intentionally lowercase for linker symbol
		// dead_code: Symbol is never directly used, only exists for linker validation
		static __reinhardt_routes_registration_marker: () = ();
	};

	Ok(expanded)
}
