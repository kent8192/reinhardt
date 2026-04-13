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
//! # Supported Function Signatures
//!
//! The macro supports three function forms:
//!
//! ## 1. Sync function (no `#[inject]`)
//!
//! ```rust,ignore
//! #[routes]
//! pub fn routes() -> UnifiedRouter {
//!     UnifiedRouter::new()
//! }
//! ```
//!
//! ## 2. Async function (no `#[inject]`)
//!
//! ```rust,ignore
//! #[routes]
//! pub async fn routes() -> UnifiedRouter {
//!     UnifiedRouter::new()
//! }
//! ```
//!
//! ## 3. Async function with `#[inject]` parameters
//!
//! ```rust,ignore
//! #[routes]
//! pub async fn routes(#[inject] router: UnifiedRouter) -> UnifiedRouter {
//!     router
//! }
//! ```
//!
//! # Generated Code
//!
//! The macro preserves the original function and adds `inventory::submit!`
//! registration code. The generated code is feature-independent to avoid
//! feature context mismatches between the library and downstream crates.
//!
//! For sync functions, a sync `RouterFactory::Sync` is registered.
//! For async functions, an async `RouterFactory::Async` is registered,
//! which returns a `Pin<Box<dyn Future>>` wrapping the async call.

use crate::crate_paths::{get_reinhardt_crate, get_reinhardt_di_crate};
use proc_macro2::TokenStream;
use quote::quote;
use syn::{FnArg, ItemFn, Pat, PatType, Result};

/// Check if an attribute is `#[inject]`
fn is_inject_attr(attr: &syn::Attribute) -> bool {
	attr.path().is_ident("inject")
}

/// Extract the inner type `T` from `Depends<T>`.
///
/// Returns `Some(T)` if the type is `Depends<T>`, `None` otherwise.
fn extract_depends_inner_type(ty: &syn::Type) -> Option<&syn::Type> {
	if let syn::Type::Path(type_path) = ty {
		let last_segment = type_path.path.segments.last()?;
		if last_segment.ident == "Depends"
			&& let syn::PathArguments::AngleBracketed(args) = &last_segment.arguments
			&& args.args.len() == 1
			&& let syn::GenericArgument::Type(inner) = args.args.first()?
		{
			return Some(inner);
		}
	}
	None
}

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
/// # Supported cases
///
/// | Case | Detection | Generated wrapper |
/// |------|-----------|-------------------|
/// | Sync, no `#[inject]` | `!async && no inject` | `RouterFactory::Sync` (unchanged) |
/// | Async, no `#[inject]` | `async && no inject` | `RouterFactory::Async` wrapper |
/// | Async, with `#[inject]` | `async && has inject` | `RouterFactory::Async` with DI context |
///
/// Sync + `#[inject]` produces a compile error.
///
/// # Parameters
///
/// * `args` - Attribute arguments. Accepts `standalone` to skip `url_prelude`
///   generation (for projects that don't use `installed_apps!`)
/// * `input` - The function to annotate
///
/// # Returns
///
/// Generated code as a `TokenStream`
///
/// # Errors
///
/// Returns an error if the function signature is invalid (e.g., missing return type,
/// sync function with `#[inject]` parameters)
pub(crate) fn routes_impl(args: TokenStream, input: ItemFn) -> Result<TokenStream> {
	// Parse optional argument: #[routes] or #[routes(standalone)]
	let standalone = if args.is_empty() {
		false
	} else {
		let ident: syn::Ident = syn::parse2(args)?;
		if ident == "standalone" {
			true
		} else {
			return Err(syn::Error::new_spanned(
				ident,
				"unknown argument for #[routes]; expected `standalone` or no arguments",
			));
		}
	};

	let reinhardt = get_reinhardt_crate();

	let fn_name = &input.sig.ident;
	let fn_vis = &input.vis;
	let fn_attrs = &input.attrs;
	let fn_block = &input.block;

	// Validate that the function has a return type
	if matches!(input.sig.output, syn::ReturnType::Default) {
		return Err(syn::Error::new_spanned(
			&input.sig,
			"#[routes] function must have a return type (-> UnifiedRouter)",
		));
	}

	let is_async = input.sig.asyncness.is_some();

	// Analyze function parameters for #[inject]
	let mut inject_params = Vec::new();
	let mut has_inject = false;

	for arg in &input.sig.inputs {
		if let FnArg::Typed(PatType { attrs, pat, ty, .. }) = arg
			&& attrs.iter().any(is_inject_attr)
		{
			has_inject = true;
			inject_params.push((pat.clone(), ty.clone()));
		}
	}

	// Sync + #[inject] is not supported (DI resolution is inherently async)
	if !is_async && has_inject {
		return Err(syn::Error::new_spanned(
			&input.sig,
			"Sync #[routes] functions cannot use #[inject] parameters. \
			 Make the function async to use dependency injection.",
		));
	}

	let expanded = if !is_async {
		// Case 1: Sync, no #[inject] — existing behavior unchanged
		let fn_sig = &input.sig;
		quote! {
			// private_interfaces: The macro forces `pub` visibility, but users
			// legitimately use `pub(crate)` newtype wrappers for DI parameters
			// (see #3498, #3468 DI pseudo orphan rule).
			#[allow(private_interfaces)]
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
			#[doc(hidden)]
			#[unsafe(no_mangle)]
			#[allow(non_upper_case_globals, dead_code)]
			// non_upper_case_globals: Intentionally lowercase for linker symbol
			// dead_code: Symbol is never directly used, only exists for linker validation
			static __reinhardt_routes_registration_marker: () = ();
		}
	} else if !has_inject {
		// Case 2: Async, no #[inject]
		let fn_sig = &input.sig;
		quote! {
			#[allow(private_interfaces)]
			#(#fn_attrs)*
			#fn_vis #fn_sig #fn_block

			#[allow(unsafe_attr_outside_unsafe)]
			const _: () = {
				fn __get_server_router() -> ::std::pin::Pin<
					::std::boxed::Box<
						dyn ::std::future::Future<
								Output = ::std::result::Result<
									::std::sync::Arc<#reinhardt::ServerRouter>,
									::std::boxed::Box<dyn ::std::error::Error + Send + Sync>,
								>,
							> + Send,
					>,
				> {
					::std::boxed::Box::pin(async {
						let unified = #fn_name().await;
						::std::result::Result::Ok(::std::sync::Arc::new(unified.into_server()))
					})
				}

				#reinhardt::inventory::submit! {
					#reinhardt::UrlPatternsRegistration::__macro_new_async(__get_server_router)
				}
			};

			#[doc(hidden)]
			#[unsafe(no_mangle)]
			#[allow(non_upper_case_globals, dead_code)]
			static __reinhardt_routes_registration_marker: () = ();
		}
	} else {
		// Case 3: Async, with #[inject]
		let di_crate = get_reinhardt_di_crate();

		// Generate dependency resolution code
		let inject_resolutions: Vec<_> = inject_params
			.iter()
			.map(|(pat, ty)| {
				if let Some(inner_ty) = extract_depends_inner_type(ty) {
					// Parameter is Depends<T>: resolve via registry only.
					// Factory-produced types (via #[injectable_factory]) do not implement
					// Injectable, so resolve_from_registry() is used to avoid requiring the bound.
					quote! {
						let #pat: #ty = #di_crate::Depends::<#inner_ty>::resolve_from_registry(&*__ctx, true).await
							.map_err(|e| -> ::std::boxed::Box<dyn ::std::error::Error + Send + Sync> {
								::std::boxed::Box::new(e)
							})?;
					}
				} else {
					// Parameter is T: resolve T, unwrap Arc<T> via clone
					quote! {
						let #pat: #ty = {
							let __arc = __ctx.resolve::<#ty>().await
								.map_err(|e| -> ::std::boxed::Box<dyn ::std::error::Error + Send + Sync> {
									::std::boxed::Box::new(e)
								})?;
							(*__arc).clone()
						};
					}
				}
			})
			.collect();

		// Generate parameter names for the call
		let inject_param_names: Vec<_> = inject_params
			.iter()
			.map(|(pat, _)| {
				if let Pat::Ident(pat_ident) = pat.as_ref() {
					let ident = &pat_ident.ident;
					quote! { #ident }
				} else {
					quote! { #pat }
				}
			})
			.collect();

		// Strip #[inject] from original function params
		let fn_return = &input.sig.output;
		let fn_generics = &input.sig.generics;
		let stripped_params: Vec<_> = input
			.sig
			.inputs
			.iter()
			.map(|arg| {
				if let FnArg::Typed(pat_type) = arg {
					let attrs: Vec<_> = pat_type
						.attrs
						.iter()
						.filter(|a| !is_inject_attr(a))
						.collect();
					let pat = &pat_type.pat;
					let ty = &pat_type.ty;
					quote! { #(#attrs)* #pat: #ty }
				} else {
					quote! { #arg }
				}
			})
			.collect();

		quote! {
			#[allow(private_interfaces)]
			#(#fn_attrs)*
			#fn_vis async fn #fn_name #fn_generics(#(#stripped_params),*) #fn_return #fn_block

			#[allow(unsafe_attr_outside_unsafe)]
			const _: () = {
				fn __get_server_router() -> ::std::pin::Pin<
					::std::boxed::Box<
						dyn ::std::future::Future<
								Output = ::std::result::Result<
									::std::sync::Arc<#reinhardt::ServerRouter>,
									::std::boxed::Box<dyn ::std::error::Error + Send + Sync>,
								>,
							> + Send,
					>,
				> {
					::std::boxed::Box::pin(async {
						// Create DI context for resolving #[inject] parameters
						let __scope = ::std::sync::Arc::new(
							#di_crate::SingletonScope::new()
						);
						let __ctx = ::std::sync::Arc::new(
							#di_crate::InjectionContext::builder(__scope).build()
						);

						// Resolve #[inject] dependencies
						#(#inject_resolutions)*

						let unified = #fn_name(#(#inject_param_names),*).await;
						::std::result::Result::Ok(::std::sync::Arc::new(unified.into_server()))
					})
				}

				#reinhardt::inventory::submit! {
					#reinhardt::UrlPatternsRegistration::__macro_new_async(__get_server_router)
				}
			};

			#[doc(hidden)]
			#[unsafe(no_mangle)]
			#[allow(non_upper_case_globals, dead_code)]
			static __reinhardt_routes_registration_marker: () = ();
		}
	};

	// Generate url_prelude module only when not in standalone mode.
	// Standalone mode skips url_prelude generation for projects that don't
	// use `installed_apps!` (e.g., reinhardt-cloud). Fixes #3542.
	let url_prelude_code = if standalone {
		quote! {}
	} else {
		quote! {
			#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
			#[doc(hidden)]
			macro_rules! __build_url_prelude {
				($($app:ident),*) => {
					/// Prelude module re-exporting all URL resolver traits and `ResolvedUrls`.
					pub mod url_prelude {
						pub use super::ResolvedUrls;
						$(pub use crate::apps::$app::urls::url_resolvers::*;)*
					}
				};
			}

			#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
			crate::__reinhardt_for_each_app!(__build_url_prelude);
		}
	};

	// Generate ResolvedUrls struct (native-only).
	// Gate on `not(wasm)` using raw platform check because this code expands
	// in consuming crates that do not have the `native` cfg alias.
	let url_resolver_code = quote! {
		#[doc(hidden)]
		pub mod __url_resolver_support {
			/// Type-safe URL resolver backed by the global `ServerRouter`.
			///
			/// Provides URL resolution methods via extension traits generated
			/// by view macros. Import `url_prelude::*` to bring all resolver
			/// methods into scope.
			#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
			pub struct ResolvedUrls {
				router: ::std::sync::Arc<#reinhardt::ServerRouter>,
			}

			#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
			impl #reinhardt::UrlResolver for ResolvedUrls {
				fn resolve_url(&self, name: &str, params: &[(&str, &str)]) -> String {
					self.router
						.reverse(name, params)
						.unwrap_or_else(|| panic!("Route '{}' not found in router", name))
				}
			}

			#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
			impl ResolvedUrls {
				/// Create a `ResolvedUrls` from the globally registered router.
				///
				/// # Panics
				///
				/// Panics if no global router has been registered via `#[routes]`.
				pub fn from_global() -> Self {
					let router = #reinhardt::get_router()
						.expect("Global router not registered. Ensure the #[routes] function has been called.");
					Self { router }
				}

				/// Create a `ResolvedUrls` from an explicit `ServerRouter`.
				pub fn from_router(router: ::std::sync::Arc<#reinhardt::ServerRouter>) -> Self {
					Self { router }
				}
			}

			#url_prelude_code
		}
		pub use __url_resolver_support::*;
	};

	let combined = quote! {
		#expanded
		#url_resolver_code
	};

	Ok(combined)
}
