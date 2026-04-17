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
use quote::{format_ident, quote};
use syn::{FnArg, ItemFn, Pat, PatType, Result};

/// Maximum number of URL parameters supported in typed resolver methods.
const MAX_URL_PARAMS: usize = 5;

/// Generate the `macro_rules!` arms for a per-app URL resolver callback macro.
///
/// Produces arms for 0 through `MAX_URL_PARAMS` parameters. Each arm generates
/// an `impl` block on `struct_ident` with a typed method that calls
/// `resolve_call` with the appropriate parameter list.
///
/// # Parameters
///
/// * `struct_ident` — the resolver struct (e.g., `PollsUrls`)
/// * `resolve_call` — the method call expression for URL resolution
/// * `name_prefix` — the route name prefix expression (e.g.,
///   `stringify!($app_label)` for server, or a string literal for client)
/// * `use_clause` — optional trait import (e.g., `use reinhardt::UrlResolver as _`)
fn gen_resolver_callback_arms(
	struct_ident: &proc_macro2::Ident,
	resolve_call: &TokenStream,
	name_prefix: &TokenStream,
	use_clause: &TokenStream,
) -> TokenStream {
	let arms: Vec<TokenStream> = (0..=MAX_URL_PARAMS)
		.map(|n| {
			let param_matchers: Vec<TokenStream> = (1..=n)
				.map(|i| {
					let p = format_ident!("p{}", i);
					quote! { $#p:literal }
				})
				.collect();

			let fn_params: Vec<TokenStream> = (1..=n)
				.map(|i| {
					let p = format_ident!("p{}", i);
					quote! { #p: &str }
				})
				.collect();

			let pairs: Vec<TokenStream> = (1..=n)
				.map(|i| {
					let p = format_ident!("p{}", i);
					quote! { ($#p, #p) }
				})
				.collect();

			quote! {
				($app_label:ident, $method:ident, $route_name:literal, #(#param_matchers),*) => {
					impl #struct_ident<'_> {
						pub fn $method(&self #(, #fn_params)*) -> String {
							#use_clause
							self.resolver.#resolve_call(
								concat!(#name_prefix, ":", $route_name),
								&[#(#pairs),*],
							)
						}
					}
				};
			}
		})
		.collect();

	quote! { #(#arms)* }
}

/// Check if an attribute is `#[inject]`
fn is_inject_attr(attr: &syn::Attribute) -> bool {
	attr.path().is_ident("inject")
}

/// Extract the inner type `T` from `Depends<T>`.
///
/// Returns `Some(T)` if the type is `Depends<T>`, `None` otherwise.
/// A sibling copy lives in `crates/reinhardt-pages/macros/src/server_fn.rs`;
/// the two proc-macro crates cannot share code directly, so keep both copies
/// in sync.
pub(crate) fn extract_depends_inner_type(ty: &syn::Type) -> Option<&syn::Type> {
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
/// * `args` - Attribute arguments. Accepts `standalone` to skip URL resolver
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

	// Generate namespaced resolvers + url_prelude only when not in standalone mode.
	// Standalone mode skips these for projects that don't use
	// `installed_apps!`. Fixes #3542.
	//
	// Per-app URL resolver struct generation (Issue #3526).
	// Reads installed app labels from the state file written by `installed_apps!`
	// and generates directly:
	//   1. Per-app struct XxxUrls<'a> with route methods
	//   2. Accessor methods on ResolvedUrls
	//   3. url_prelude module with re-exports
	//
	// This replaces the __reinhardt_for_each_app callback pattern that triggers
	// macro_expanded_macro_exports_accessed_by_absolute_paths on Rust 1.94+.
	// Fixes #3639.
	let url_prelude_code = if standalone || crate::macro_state::is_wasm_target() {
		// Standalone mode: projects that don't use installed_apps!.
		// WASM target: URL resolvers are native-only; skip reading the state file
		// to avoid failures when installed_apps! hasn't written it for WASM builds.
		quote! {}
	} else {
		let app_labels = match crate::macro_state::read_installed_apps() {
			Ok(labels) => labels,
			Err(msg) => {
				return Err(syn::Error::new(
					proc_macro2::Span::call_site(),
					format!(
						"Failed to read installed apps: {msg}. \
						 Ensure `installed_apps!` is invoked before `#[routes]`."
					),
				));
			}
		};

		let app_idents: Vec<proc_macro2::Ident> = app_labels
			.iter()
			.filter(|s| !s.is_empty())
			.map(|s| {
				syn::parse_str::<syn::Ident>(s).map_err(|_| {
					syn::Error::new(
						proc_macro2::Span::call_site(),
						format!(
							"Invalid installed app label `{s}`: expected a valid Rust identifier"
						),
					)
				})
			})
			.collect::<Result<Vec<_>>>()?;

		if app_idents.is_empty() {
			quote! {
				#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
				/// Prelude module re-exporting URL resolver types.
				pub mod url_prelude {
					pub use super::ResolvedUrls;
				}
			}
		} else {
			// Generate per-app resolver structs and methods.
			// The callback macro __gen_<app>_method creates impl blocks
			// for each route discovered via __for_each_url_resolver.
			let per_app_code: Vec<_> = app_idents
				.iter()
				.map(|app| {
					let urls_struct_name =
						crate::pascal_case::to_pascal_case_with_suffix(&app.to_string(), "Urls");
					let urls_struct =
						proc_macro2::Ident::new(&urls_struct_name, proc_macro2::Span::call_site());
					let gen_method_macro = proc_macro2::Ident::new(
						&format!("__gen_{}_method", app),
						proc_macro2::Span::call_site(),
					);

					let server_callback_arms = gen_resolver_callback_arms(
						&urls_struct,
						&quote! { resolve_url },
						&quote! { stringify!($app_label) },
						&quote! { use #reinhardt::UrlResolver as _; },
					);

					quote! {
						/// Per-app URL resolver.
						///
						/// Access via `ResolvedUrls::#app()`.
						pub struct #urls_struct<'a> {
							resolver: &'a ResolvedUrls,
						}

						// Callback macro for __for_each_url_resolver to generate methods.
						// Each arm imports UrlResolver trait to bring resolve_url() into
						// scope. (Issue #3669)
						// Arms generated by gen_resolver_callback_arms() for 0..=5 params.
						macro_rules! #gen_method_macro {
							#server_callback_arms
						}

						// Invoke __for_each_url_resolver to populate methods.
						// Pass the absolute path to `url_resolvers` as `$base`
						// so that metadata macros resolve correctly at the call site.
						crate::apps::#app::urls::url_resolvers::__for_each_url_resolver!(
							#gen_method_macro, #app,
							crate::apps::#app::urls::url_resolvers
						);

						// Accessor method on ResolvedUrls
						impl ResolvedUrls {
							pub fn #app(&self) -> #urls_struct<'_> {
								#urls_struct { resolver: self }
							}
						}
					}
				})
				.collect();

			// Generate per-app client URL resolver structs.
			// When #[url_patterns(InstalledApp::<variant>, mode = client)] is used,
			// typed methods are generated via __for_each_client_url_resolver
			// (same pattern as server-side). A fallback resolve() method is
			// always available for runtime string-based resolution.
			let per_app_client_code: Vec<_> = app_idents
				.iter()
				.map(|app| {
					let client_urls_struct_name = crate::pascal_case::to_pascal_case_with_suffix(
						&app.to_string(),
						"ClientUrls",
					);
					let client_urls_struct = proc_macro2::Ident::new(
						&client_urls_struct_name,
						proc_macro2::Span::call_site(),
					);
					let gen_client_method_macro = proc_macro2::Ident::new(
						&format!("__gen_{}_client_method", app),
						proc_macro2::Span::call_site(),
					);
					let accessor_method = proc_macro2::Ident::new(
						&format!("{}_client", app),
						proc_macro2::Span::call_site(),
					);
					let app_str = app.to_string();

					let client_callback_arms = gen_resolver_callback_arms(
						&client_urls_struct,
						&quote! { resolve_client_url },
						&quote! { #app_str },
						&quote! {},
					);

					quote! {
						/// Per-app client URL resolver.
						///
						/// Access via `ResolvedUrls::#accessor_method()`.
						pub struct #client_urls_struct<'a> {
							resolver: &'a ResolvedUrls,
						}

						impl #client_urls_struct<'_> {
							/// Resolve a client-side URL by route name and parameters.
							///
							/// Fallback for routes not covered by typed methods.
							/// The route name is automatically prefixed with the app label.
							pub fn resolve(&self, route_name: &str, params: &[(&str, &str)]) -> String {
								let full_name = ::std::format!("{}:{}", #app_str, route_name);
								self.resolver.resolve_client_url(&full_name, params)
							}
						}

						// Callback macro for __for_each_client_url_resolver to generate
						// typed methods (same pattern as server-side __gen_<app>_method).
						// Arms generated by gen_resolver_callback_arms() for 0..=5 params.
						macro_rules! #gen_client_method_macro {
							#client_callback_arms
						}

						// Invoke __for_each_client_url_resolver to populate typed methods.
						// This is a no-op if the app has no client_url_resolvers module
						// (i.e., does not use #[url_patterns(..., mode = client)] or
						// #[url_patterns(..., mode = unified)]).
						crate::apps::#app::urls::client_url_resolvers::__for_each_client_url_resolver!(
							#gen_client_method_macro, #app,
							crate::apps::#app::urls::client_url_resolvers
						);

						// Accessor method on ResolvedUrls
						impl ResolvedUrls {
							pub fn #accessor_method(&self) -> #client_urls_struct<'_> {
								#client_urls_struct { resolver: self }
							}
						}
					}
				})
				.collect();

			// Generate url_prelude re-exports
			let prelude_exports: Vec<_> = app_idents
				.iter()
				.map(|app| {
					let urls_struct_name =
						crate::pascal_case::to_pascal_case_with_suffix(&app.to_string(), "Urls");
					let urls_struct =
						proc_macro2::Ident::new(&urls_struct_name, proc_macro2::Span::call_site());
					let client_urls_struct_name = crate::pascal_case::to_pascal_case_with_suffix(
						&app.to_string(),
						"ClientUrls",
					);
					let client_urls_struct = proc_macro2::Ident::new(
						&client_urls_struct_name,
						proc_macro2::Span::call_site(),
					);
					quote! {
						pub use super::#urls_struct;
						#[cfg(feature = "client-router")]
						pub use super::super::__namespaced_client_resolvers::#client_urls_struct;
						// Deprecated flat trait re-exports (backward compatibility)
						#[allow(deprecated)]
						pub use crate::apps::#app::urls::url_resolvers::*;
					}
				})
				.collect();

			quote! {
				// Track state file for incremental compilation invalidation.
				// When installed_apps! rewrites the file, the compiler re-expands #[routes].
				#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
				const _: &[u8] = include_bytes!(
					concat!(env!("CARGO_MANIFEST_DIR"), "/target/reinhardt/.installed_apps")
				);

				// Server-side per-app resolvers are native-only.
				#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
				#[doc(hidden)]
				mod __namespaced_resolvers {
					#![allow(unexpected_cfgs)]
					pub use super::ResolvedUrls;

					#(#per_app_code)*

					/// Prelude module re-exporting URL resolver types.
					pub mod url_prelude {
						pub use super::ResolvedUrls;
						#(#prelude_exports)*
					}
				}
				#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
				pub use __namespaced_resolvers::*;

				// Client-side per-app resolvers are cross-platform (native + WASM).
				#[doc(hidden)]
				mod __client_router_gate {
					#![allow(unexpected_cfgs)]

					#[cfg(feature = "client-router")]
					#[doc(hidden)]
					pub mod __namespaced_client_resolvers {
						pub use super::super::ResolvedUrls;

						#(#per_app_client_code)*
					}
					#[cfg(feature = "client-router")]
					pub use __namespaced_client_resolvers::*;
				}
				pub use __client_router_gate::*;
			}
		}
	};

	// Generate ResolvedUrls struct.
	// Native: holds both ServerRouter and ClientUrlReverser.
	// WASM: holds only ClientUrlReverser.
	// Gate using raw platform check because this code expands in consuming
	// crates that do not have the `native` cfg alias.
	let url_resolver_code = quote! {
		#[doc(hidden)]
		pub mod __url_resolver_support {
			#![allow(unexpected_cfgs)]
			/// Type-safe URL resolver backed by the global `ServerRouter`
			/// and `ClientUrlReverser`.
			///
			/// Provides URL resolution methods via extension traits generated
			/// by view macros. Import `url_prelude::*` to bring all resolver
			/// methods into scope.
			#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
			pub struct ResolvedUrls {
				router: ::std::sync::Arc<#reinhardt::ServerRouter>,
				#[cfg(feature = "client-router")]
				client_reverser: ::std::sync::Arc<#reinhardt::ClientUrlReverser>,
			}

			/// WASM-only `ResolvedUrls` with client URL resolution only.
			#[cfg(all(target_family = "wasm", target_os = "unknown"))]
			pub struct ResolvedUrls {
				client_reverser: ::std::sync::Arc<#reinhardt::ClientUrlReverser>,
			}

			#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
			impl #reinhardt::UrlResolver for ResolvedUrls {
				fn resolve_url(&self, name: &str, params: &[(&str, &str)]) -> String {
					self.router
						.reverse(name, params)
						.unwrap_or_else(|| panic!("Route '{}' not found in router", name))
				}
			}

			#[cfg(feature = "client-router")]
			impl #reinhardt::ClientUrlResolver for ResolvedUrls {
				fn resolve_client_url(&self, name: &str, params: &[(&str, &str)]) -> String {
					self.client_reverser
						.reverse(name, params)
						.unwrap_or_else(|| panic!("Client route '{}' not found in router", name))
				}
			}

			#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
			impl ResolvedUrls {
				/// Create a `ResolvedUrls` from the globally registered router.
				///
				/// # Panics
				///
				/// Panics if no global router has been registered via `#[routes]`.
				#[cfg(feature = "client-router")]
				pub fn from_global() -> Self {
					let router = #reinhardt::get_router()
						.expect("Global router not registered. Ensure the #[routes] function has been called.");
					let client_reverser = #reinhardt::get_client_reverser()
						.expect("Global client reverser not registered. Ensure the #[routes] function has been called.");
					Self { router, client_reverser }
				}

				/// Create a `ResolvedUrls` from the globally registered router (without client router).
				///
				/// # Panics
				///
				/// Panics if no global router has been registered via `#[routes]`.
				#[cfg(not(feature = "client-router"))]
				pub fn from_global() -> Self {
					let router = #reinhardt::get_router()
						.expect("Global router not registered. Ensure the #[routes] function has been called.");
					Self { router }
				}

				/// Create a `ResolvedUrls` from explicit router and client reverser.
				#[cfg(feature = "client-router")]
				pub fn from_router(
					router: ::std::sync::Arc<#reinhardt::ServerRouter>,
					client_reverser: ::std::sync::Arc<#reinhardt::ClientUrlReverser>,
				) -> Self {
					Self { router, client_reverser }
				}

				/// Create a `ResolvedUrls` from an explicit router (without client router).
				#[cfg(not(feature = "client-router"))]
				pub fn from_router(
					router: ::std::sync::Arc<#reinhardt::ServerRouter>,
				) -> Self {
					Self { router }
				}
			}

			#[cfg(all(target_family = "wasm", target_os = "unknown"))]
			impl ResolvedUrls {
				/// Create a `ResolvedUrls` from the globally registered client reverser.
				///
				/// # Panics
				///
				/// Panics if no global client reverser has been registered via `#[routes]`.
				pub fn from_global() -> Self {
					let client_reverser = #reinhardt::get_client_reverser()
						.expect("Global client reverser not registered. Ensure the #[routes] function has been called.");
					Self { client_reverser }
				}

				/// Create a `ResolvedUrls` from an explicit client reverser.
				pub fn from_reverser(
					client_reverser: ::std::sync::Arc<#reinhardt::ClientUrlReverser>,
				) -> Self {
					Self { client_reverser }
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
