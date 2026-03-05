//! Use inject macro for automatic dependency injection
//!
//! Provides FastAPI-style dependency injection via function parameter attributes.
//!
//! # Router Compatibility
//!
//! The generated wrapper function has signature `Fn(Request) -> Fut` to be compatible
//! with `UnifiedRouter::function()`. The `InjectionContext` is extracted from
//! `Request.get_di_context()` which is set by the router before dispatching.

use crate::crate_paths::{
	get_reinhardt_core_crate, get_reinhardt_di_crate, get_reinhardt_http_crate,
};
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Attribute, FnArg, ItemFn, Pat, PatType, Result, Type};

/// Check if an attribute is `#[inject]`
fn is_inject_attr(attr: &Attribute) -> bool {
	attr.path().is_ident("inject")
}

/// Process a function argument and determine if it needs injection
#[derive(Clone)]
struct ProcessedArg {
	/// Original pattern (variable name)
	pat: Pat,
	/// Type of the argument
	ty: Type,
	/// Whether this argument should be injected
	inject: bool,
	/// Whether to use cache (default: true)
	use_cache: bool,
}

impl ProcessedArg {
	fn from_fn_arg(arg: &FnArg) -> Option<Self> {
		match arg {
			FnArg::Typed(PatType { attrs, pat, ty, .. }) => {
				let mut inject = false;
				let mut use_cache = true;

				// Check for #[inject] or #[inject(cache = false)]
				for attr in attrs {
					if is_inject_attr(attr) {
						inject = true;

						// Parse arguments like #[inject(cache = false)]
						if let Ok(meta) = attr.parse_args::<syn::Meta>()
							&& let syn::Meta::NameValue(nv) = meta
							&& nv.path.is_ident("cache")
							&& let syn::Expr::Lit(syn::ExprLit {
								lit: syn::Lit::Bool(lit_bool),
								..
							}) = &nv.value
						{
							use_cache = lit_bool.value;
						}
					}
				}

				// Remove `mut` modifier from pattern if present
				let pat_without_mut = match &**pat {
					syn::Pat::Ident(pat_ident) => {
						let mut new_pat_ident = pat_ident.clone();
						new_pat_ident.mutability = None;
						syn::Pat::Ident(new_pat_ident)
					}
					other => other.clone(),
				};

				Some(ProcessedArg {
					pat: pat_without_mut,
					ty: (**ty).clone(),
					inject,
					use_cache,
				})
			}
			_ => None,
		}
	}
}
/// Implementation of the `use_inject` procedural macro
///
/// This function is used internally by the `#[use_inject]` attribute macro.
/// Users should not call this function directly.
///
/// Generates a wrapper function that:
/// 1. Has signature `Fn(Request) -> impl Future<Output = Result>` (router-compatible)
/// 2. Extracts `InjectionContext` from `Request.get_di_context()`
/// 3. Resolves `#[inject]` dependencies from the context
/// 4. Calls the original function
///
/// For methods (with &self), the wrapper also takes &self and calls self.original_fn.
///
/// # Request Parameter
///
/// The Request parameter is optional:
/// - If present: DI context is extracted from that request
/// - If not present: A Request parameter is automatically added to the wrapper
pub(crate) fn use_inject_impl(_args: TokenStream, input: ItemFn) -> Result<TokenStream> {
	let di_crate = get_reinhardt_di_crate();
	let core_crate = get_reinhardt_core_crate();
	let http_crate = get_reinhardt_http_crate();

	let ItemFn {
		attrs,
		vis,
		sig,
		block,
	} = input;

	let fn_name = &sig.ident;
	let asyncness = &sig.asyncness;
	let output = &sig.output;
	let generics = &sig.generics;
	let where_clause = &sig.generics.where_clause;

	// Validate that function is async
	if asyncness.is_none() {
		return Err(syn::Error::new_spanned(
			&sig,
			"#[use_inject] can only be used on async functions",
		));
	}

	// Validate return type exists
	if matches!(sig.output, syn::ReturnType::Default) {
		return Err(syn::Error::new_spanned(
			&sig,
			"#[use_inject] functions must have an explicit return type",
		));
	}

	// Process all function arguments
	let mut processed_args = Vec::new();
	let mut self_param: Option<FnArg> = None;
	let mut inject_params = Vec::new();
	let mut request_param = None;
	let mut other_params = Vec::new(); // Non-Request, non-inject parameters

	for arg in &sig.inputs {
		if let Some(processed) = ProcessedArg::from_fn_arg(arg) {
			if processed.inject {
				inject_params.push(processed);
			} else {
				// Check if this is the Request parameter
				let is_request = if let Type::Path(type_path) = &processed.ty {
					type_path
						.path
						.segments
						.last()
						.is_some_and(|s| s.ident == "Request")
				} else {
					false
				};

				if is_request {
					request_param = Some(processed.pat.clone());
				} else {
					// This is a regular parameter (not Request, not inject)
					other_params.push(processed.clone());
				}
				processed_args.push(processed);
			}
		} else {
			// This is a self parameter
			self_param = Some(arg.clone());
		}
	}

	// Determine if Request parameter exists
	let has_request = request_param.is_some();

	// Use existing request pat or create a new one
	let request_pat: Pat = request_param.unwrap_or_else(|| {
		syn::parse_quote! { __req }
	});

	// Build original function name
	let original_fn_name = syn::Ident::new(&format!("{}_original", fn_name), fn_name.span());

	// Build the original function signature (all parameters including #[inject])
	let mut original_params: Vec<FnArg> = Vec::new();
	if let Some(ref self_p) = self_param {
		original_params.push(self_p.clone());
	}
	for arg in &processed_args {
		let pat = &arg.pat;
		let ty = &arg.ty;
		original_params.push(syn::parse_quote! { #pat: #ty });
	}
	for arg in &inject_params {
		let pat = &arg.pat;
		let ty = &arg.ty;
		original_params.push(syn::parse_quote! { #pat: #ty });
	}

	// Generate DI context extraction (from Request)
	let di_context_extraction = if !inject_params.is_empty() {
		quote! {
			let __di_ctx = #request_pat.get_di_context::<::std::sync::Arc<#di_crate::InjectionContext>>()
				.ok_or_else(|| #core_crate::exception::Error::Internal(
					"DI context not set. Ensure the router is configured with .with_di_context()".to_string()
				))?;
		}
	} else {
		quote! {}
	};

	// Generate injection code for wrapper
	let mut injection_stmts = Vec::new();
	for arg in &inject_params {
		let pat = &arg.pat;
		let ty = &arg.ty;

		let injection_code = if arg.use_cache {
			quote! {
				let #pat: #ty = #di_crate::Injected::<#ty>::resolve(&__di_ctx)
					.await
					.map_err(|e| {
						#core_crate::exception::Error::Internal(
							format!("Dependency injection failed: {:?}", e)
						)
					})?
					.into_inner();
			}
		} else {
			quote! {
				let #pat: #ty = #di_crate::Injected::<#ty>::resolve_uncached(&__di_ctx)
					.await
					.map_err(|e| {
						#core_crate::exception::Error::Internal(
							format!("Dependency injection failed: {:?}", e)
						)
					})?
					.into_inner();
			}
		};

		injection_stmts.push(injection_code);
	}

	// Extract argument patterns for the original function call
	// Only include processed_args (non-inject) for the call, inject_params are resolved separately
	let call_args: Vec<_> = processed_args
		.iter()
		.chain(inject_params.iter())
		.map(|arg| &arg.pat)
		.collect();

	// Determine if this is a method (has self parameter)
	let is_method = self_param.is_some();

	// Generate the call to the original function
	let original_call = if is_method {
		quote! { self.#original_fn_name(#(#call_args),*).await }
	} else {
		quote! { #original_fn_name(#(#call_args),*).await }
	};

	// Build other_params tokens for wrapper function
	let other_param_tokens: Vec<_> = other_params
		.iter()
		.map(|arg| {
			let pat = &arg.pat;
			let ty = &arg.ty;
			quote! { #pat: #ty }
		})
		.collect();

	// Build wrapper function parameters
	// Signature: (&self,)? request: Request (router-compatible, no DI context parameter)
	// Note: DI context is extracted from Request.di_context() inside the wrapper
	// If original function had Request, it's included; otherwise we add it for DI extraction
	let wrapper_params = if is_method {
		let self_p = self_param.as_ref().unwrap();
		if has_request {
			// Request was in original signature
			if other_param_tokens.is_empty() {
				quote! { #self_p, #request_pat: #http_crate::Request }
			} else {
				quote! { #self_p, #request_pat: #http_crate::Request, #(#other_param_tokens),* }
			}
		} else {
			// Request not in original, add it for DI
			if other_param_tokens.is_empty() {
				quote! { #self_p, #request_pat: #http_crate::Request }
			} else {
				quote! { #self_p, #request_pat: #http_crate::Request, #(#other_param_tokens),* }
			}
		}
	} else if has_request {
		// Request was in original signature
		if other_param_tokens.is_empty() {
			quote! { #request_pat: #http_crate::Request }
		} else {
			quote! { #request_pat: #http_crate::Request, #(#other_param_tokens),* }
		}
	} else {
		// Request not in original, add it for DI
		if other_param_tokens.is_empty() {
			quote! { #request_pat: #http_crate::Request }
		} else {
			quote! { #request_pat: #http_crate::Request, #(#other_param_tokens),* }
		}
	};

	// Generate the expanded code with both original and wrapper functions
	let expanded = quote! {
		// Original function (renamed, private)
		#asyncness fn #original_fn_name #generics (#(#original_params),*) #output #where_clause {
			#block
		}

		// Public wrapper function with signature Fn(Request) -> Future (router-compatible)
		#(#attrs)*
		#vis #asyncness fn #fn_name #generics (#wrapper_params) #output #where_clause {
			// Extract DI context from Request (set by router before dispatching)
			#di_context_extraction

			// Resolve #[inject] dependencies
			#(#injection_stmts)*

			// Call the original function
			#original_call
		}
	};

	Ok(expanded)
}
