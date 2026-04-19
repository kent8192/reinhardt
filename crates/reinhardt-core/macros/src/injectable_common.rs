//! Common utilities for injectable macros
//!
//! This module contains shared logic for both function-based (`#[injectable]` on functions)
//! and struct-based (`#[injectable]` on structs) dependency injection.

use crate::crate_paths::{
	get_reinhardt_core_crate, get_reinhardt_di_crate, get_reinhardt_signals_crate,
};
use syn::{Expr, Token, punctuated::Punctuated};

/// Scope for dependency injection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum InjectionScope {
	/// Request scope - dependencies are created per request (default)
	#[default]
	Request,
	/// Singleton scope - dependencies are created once and shared
	Singleton,
}

/// Parsed options from `#[inject(...)]` attribute
#[derive(Debug, Clone)]
pub(crate) struct InjectOptions {
	/// Whether to use caching for dependency resolution
	pub use_cache: bool,
	/// The scope for dependency injection
	pub scope: InjectionScope,
}

impl Default for InjectOptions {
	fn default() -> Self {
		Self {
			use_cache: true,
			scope: InjectionScope::Request,
		}
	}
}

/// Default value specification for `#[no_inject]` fields
#[derive(Debug, Clone)]
pub(crate) enum DefaultValue {
	/// Use Default::default()
	DefaultTrait,
	/// Use a specific expression
	Expression(Expr),
	/// No default value specified - field must be `Option<T>`
	None,
}

/// Parsed options from `#[no_inject(...)]` attribute
#[derive(Debug, Clone)]
pub(crate) struct NoInjectOptions {
	pub default: DefaultValue,
}

/// Check if an attribute is `#[inject]`
pub(crate) fn is_inject_attr(attr: &syn::Attribute) -> bool {
	attr.path().is_ident("inject")
}

/// Check if an attribute is `#[no_inject]`
pub(crate) fn is_no_inject_attr(attr: &syn::Attribute) -> bool {
	attr.path().is_ident("no_inject")
}

/// Parse `#[inject]` or `#[inject(cache = false, scope = Singleton)]` attributes
///
/// Returns `InjectOptions` with parsed settings. If no `#[inject]` attribute is found,
/// returns default options.
pub(crate) fn parse_inject_options(attrs: &[syn::Attribute]) -> InjectOptions {
	let mut options = InjectOptions::default();

	for attr in attrs {
		if !is_inject_attr(attr) {
			continue;
		}

		// Try to parse as Meta::List: #[inject(cache = false, scope = Singleton)]
		if let syn::Meta::List(meta_list) = &attr.meta
			&& let Ok(nested) =
				meta_list.parse_args_with(Punctuated::<syn::Meta, Token![,]>::parse_terminated)
		{
			for meta in nested {
				if let syn::Meta::NameValue(nv) = meta {
					if nv.path.is_ident("cache") {
						if let syn::Expr::Lit(syn::ExprLit {
							lit: syn::Lit::Bool(lit_bool),
							..
						}) = &nv.value
						{
							options.use_cache = lit_bool.value;
						}
					} else if nv.path.is_ident("scope")
						&& let syn::Expr::Path(path_expr) = &nv.value
					{
						if path_expr.path.is_ident("Singleton") {
							options.scope = InjectionScope::Singleton;
						} else if path_expr.path.is_ident("Request") {
							options.scope = InjectionScope::Request;
						}
					}
				}
			}
		}
	}

	options
}

/// Parse `#[no_inject]` or `#[no_inject(default = ...)]` attributes
///
/// Returns `Some(NoInjectOptions)` if `#[no_inject]` attribute is found, `None` otherwise.
pub(crate) fn parse_no_inject_options(attrs: &[syn::Attribute]) -> Option<NoInjectOptions> {
	for attr in attrs {
		if !is_no_inject_attr(attr) {
			continue;
		}

		// #[no_inject] without arguments -> None default
		if matches!(&attr.meta, syn::Meta::Path(_)) {
			return Some(NoInjectOptions {
				default: DefaultValue::None,
			});
		}

		// Try to parse as Meta::List: #[no_inject(default = ...)]
		if let syn::Meta::List(meta_list) = &attr.meta
			&& let Ok(nested) =
				meta_list.parse_args_with(Punctuated::<syn::Meta, Token![,]>::parse_terminated)
		{
			for meta in nested {
				if let syn::Meta::NameValue(nv) = meta
					&& nv.path.is_ident("default")
				{
					// Check if the value is the special "Default" keyword
					if let Expr::Path(path_expr) = &nv.value
						&& path_expr.path.is_ident("Default")
					{
						return Some(NoInjectOptions {
							default: DefaultValue::DefaultTrait,
						});
					} else {
						// Any other expression
						return Some(NoInjectOptions {
							default: DefaultValue::Expression(nv.value.clone()),
						});
					}
				}
			}
		}

		// If we found #[no_inject(...)] but couldn't parse it, return None default
		return Some(NoInjectOptions {
			default: DefaultValue::None,
		});
	}

	None
}

// ============================================================================
// Code Generation Utilities for DI
// ============================================================================

use proc_macro2::TokenStream;

/// Information about `#[inject]` parameters (for code generation)
///
/// This struct is part of the DI code generation infrastructure and will be used
/// by macro extensions like `#[action]` and `#[receiver]` with use_inject support.
#[derive(Clone)]
pub(crate) struct InjectParamInfo {
	/// Parameter pattern (variable name)
	pub pat: Box<syn::Pat>,
	/// Parameter type
	pub ty: Box<syn::Type>,
	/// Inject options (cache, scope)
	pub options: InjectOptions,
}

/// Detects parameters with `#[inject]` attribute from function arguments.
///
/// This function is part of the DI code generation infrastructure and is used
/// by macro extensions like `#[action]` and `#[receiver]` with use_inject support.
///
/// # Integration
///
/// - `#[action]` macro: Controller action methods with automatic DI
/// - `#[receiver]` macro: Signal receiver functions with injected dependencies
/// - use_inject flag: Enable DI for custom macros
pub(crate) fn detect_inject_params(
	inputs: &syn::punctuated::Punctuated<syn::FnArg, Token![,]>,
) -> Vec<InjectParamInfo> {
	let mut inject_params = Vec::new();

	for input in inputs {
		if let syn::FnArg::Typed(syn::PatType { attrs, pat, ty, .. }) = input {
			let has_inject = attrs.iter().any(is_inject_attr);

			if has_inject {
				let options = parse_inject_options(attrs);
				inject_params.push(InjectParamInfo {
					pat: pat.clone(),
					ty: ty.clone(),
					options,
				});
			}
		}
	}

	inject_params
}

/// Generates DI context extraction code from a Request.
///
/// This function is used by `#[action]` and `#[receiver]` macros to extract
/// the DI context from request objects for dependency resolution.
/// Also sets up `__resolve_ctx` for use with `RESOLVE_CTX.scope()`.
pub(crate) fn generate_di_context_extraction(request_ident: &syn::Ident) -> TokenStream {
	let di_crate = get_reinhardt_di_crate();
	let core_crate = get_reinhardt_core_crate();

	quote::quote! {
		let __shared_ctx = #request_ident.get_di_context::<::std::sync::Arc<#di_crate::InjectionContext>>()
			.ok_or_else(|| #core_crate::exception::Error::Internal(
				"DI context not set. Ensure the router is configured with .with_di_context()".to_string()
			))?;
		let __di_request = #request_ident.clone_for_di();
		let __di_ctx = ::std::sync::Arc::new((*__shared_ctx).fork_for_request(__di_request));
		let __resolve_ctx = #di_crate::resolve_context::ResolveContext {
			root: ::std::sync::Arc::clone(&__shared_ctx),
			current: ::std::sync::Arc::clone(&__di_ctx),
		};
	}
}

/// Generates DI context extraction code from an optional `Arc<InjectionContext>`.
///
/// Used for Signal receivers where DI context is passed as an Option.
/// This function enables `#[receiver]` macros to handle optional DI contexts
/// in signal dispatch scenarios.
pub(crate) fn generate_di_context_extraction_from_option(ctx_ident: &syn::Ident) -> TokenStream {
	let signals_crate = get_reinhardt_signals_crate();

	quote::quote! {
		let __di_ctx = #ctx_ident
			.as_ref()
			.ok_or_else(|| #signals_crate::SignalError::new(
				"DI context not available. Use signal.send_with_di_context() to enable injection"
			))?;
	}
}

/// Generates injection resolution calls for a list of inject parameters.
///
/// This function is used by `#[action]` and `#[receiver]` macros to generate
/// dependency injection code for parameters marked with `#[inject]`.
pub(crate) fn generate_injection_calls(inject_params: &[InjectParamInfo]) -> Vec<TokenStream> {
	let di_crate = get_reinhardt_di_crate();
	let core_crate = get_reinhardt_core_crate();

	inject_params
		.iter()
		.map(|param| {
			let pat = &param.pat;
			let ty = &param.ty;
			let use_cache = param.options.use_cache;

			if use_cache {
				quote::quote! {
					let #pat: #ty = #di_crate::Depends::<#ty>::resolve(&__di_ctx, true)
						.await
						.map_err(|e| #core_crate::exception::Error::Internal(
							format!("Dependency injection failed for {}: {:?}", stringify!(#ty), e)
						))?
						.into_inner();
				}
			} else {
				quote::quote! {
					let #pat: #ty = #di_crate::Depends::<#ty>::resolve(&__di_ctx, false)
						.await
						.map_err(|e| #core_crate::exception::Error::Internal(
							format!("Dependency injection failed for {}: {:?}", stringify!(#ty), e)
						))?
						.into_inner();
				}
			}
		})
		.collect()
}

/// Generates injection resolution calls with a custom error type.
///
/// Used for WebSocket handlers and Signal receivers that use different error types.
/// This function enables `#[action]` and `#[receiver]` macros to generate error
/// handling code compatible with their specific error types.
pub(crate) fn generate_injection_calls_with_error<F>(
	inject_params: &[InjectParamInfo],
	error_mapper: F,
) -> Vec<TokenStream>
where
	F: Fn(&syn::Type) -> TokenStream,
{
	let di_crate = get_reinhardt_di_crate();

	inject_params
		.iter()
		.map(|param| {
			let pat = &param.pat;
			let ty = &param.ty;
			let use_cache = param.options.use_cache;
			let error_conversion = error_mapper(ty);

			if use_cache {
				quote::quote! {
					let #pat: #ty = #di_crate::Depends::<#ty>::resolve(&__di_ctx, true)
						.await
						.map_err(|e| #error_conversion)?
						.into_inner();
				}
			} else {
				quote::quote! {
					let #pat: #ty = #di_crate::Depends::<#ty>::resolve(&__di_ctx, false)
						.await
						.map_err(|e| #error_conversion)?
						.into_inner();
				}
			}
		})
		.collect()
}

/// Removes `#[inject]` attributes from function arguments.
///
/// Returns a new list of FnArg with `#[inject]` attributes stripped.
/// This function is used by `#[action]` and `#[receiver]` macros to clean up
/// function signatures after processing `#[inject]` attributes for code generation.
pub(crate) fn strip_inject_attrs(
	inputs: &syn::punctuated::Punctuated<syn::FnArg, Token![,]>,
) -> Vec<syn::FnArg> {
	inputs
		.iter()
		.map(|arg| {
			if let syn::FnArg::Typed(pat_type) = arg {
				let mut pat_type = pat_type.clone();
				pat_type.attrs.retain(|attr| !is_inject_attr(attr));
				syn::FnArg::Typed(pat_type)
			} else {
				arg.clone()
			}
		})
		.collect()
}
