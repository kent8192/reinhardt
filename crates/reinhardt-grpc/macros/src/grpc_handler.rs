//! gRPC handler macro implementation
//!
//! Provides the `#[grpc_handler]` attribute macro for gRPC service methods
//! that use dependency injection.

use crate::crate_paths::{get_reinhardt_di_crate, get_reinhardt_grpc_crate};
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{Error, FnArg, ItemFn, Pat, PatType, Result, Token, Type, punctuated::Punctuated};

/// Information about parameter extractors
#[derive(Clone)]
struct ParamInfo {
	pat: Box<Pat>,
	ty: Box<Type>,
}

/// Information about `#[inject]` parameters
#[derive(Clone)]
struct InjectInfo {
	pat: Box<Pat>,
	ty: Box<Type>,
	use_cache: bool,
}

/// Options for `#[inject]` attribute
#[derive(Clone, Default)]
struct InjectOptions {
	use_cache: bool,
}

/// Check if an attribute is `#[inject]`
fn is_inject_attr(attr: &syn::Attribute) -> bool {
	attr.path().is_ident("inject")
}

/// Known option names for `#[inject(...)]`
const KNOWN_INJECT_OPTIONS: &[&str] = &["cache"];

/// Parse `#[inject]` or `#[inject(cache = false)]` attributes
///
/// Returns an error for unrecognized options or invalid value types.
fn parse_inject_options(attrs: &[syn::Attribute]) -> Result<InjectOptions> {
	let mut options = InjectOptions {
		use_cache: true, // Default to caching enabled
	};

	for attr in attrs {
		if !is_inject_attr(attr) {
			continue;
		}

		// #[inject] without arguments - use defaults
		if matches!(&attr.meta, syn::Meta::Path(_)) {
			continue;
		}

		// Parse as Meta::List: #[inject(cache = false)]
		let syn::Meta::List(meta_list) = &attr.meta else {
			continue;
		};

		let nested =
			meta_list.parse_args_with(Punctuated::<syn::Meta, Token![,]>::parse_terminated)?;

		for meta in &nested {
			match meta {
				syn::Meta::NameValue(nv) if nv.path.is_ident("cache") => {
					if let syn::Expr::Lit(syn::ExprLit {
						lit: syn::Lit::Bool(lit_bool),
						..
					}) = &nv.value
					{
						options.use_cache = lit_bool.value;
					} else {
						return Err(Error::new_spanned(
							&nv.value,
							"`cache` option expects a boolean value (e.g., `cache = false`)",
						));
					}
				}
				syn::Meta::NameValue(nv) => {
					let name = nv
						.path
						.get_ident()
						.map(ToString::to_string)
						.unwrap_or_else(|| "<unknown>".to_string());
					return Err(Error::new_spanned(
						&nv.path,
						format!(
							"unrecognized `inject` option `{name}`. \
							 Valid options: {}",
							KNOWN_INJECT_OPTIONS
								.iter()
								.map(|o| format!("`{o}`"))
								.collect::<Vec<_>>()
								.join(", ")
						),
					));
				}
				other => {
					let name = other
						.path()
						.get_ident()
						.map(ToString::to_string)
						.unwrap_or_else(|| "<unknown>".to_string());
					return Err(Error::new_spanned(
						other.path(),
						format!(
							"unrecognized `inject` option `{name}`. \
							 Valid options: {}",
							KNOWN_INJECT_OPTIONS
								.iter()
								.map(|o| format!("`{o}`"))
								.collect::<Vec<_>>()
								.join(", ")
						),
					));
				}
			}
		}
	}

	Ok(options)
}

/// Detect parameters with `#[inject]` attribute
fn detect_inject_params(inputs: &Punctuated<FnArg, Token![,]>) -> Result<Vec<InjectInfo>> {
	let mut inject_params = Vec::new();

	for input in inputs {
		if let FnArg::Typed(PatType { attrs, pat, ty, .. }) = input {
			let has_inject = attrs.iter().any(is_inject_attr);

			if has_inject {
				let options = parse_inject_options(attrs)?;
				inject_params.push(InjectInfo {
					pat: pat.clone(),
					ty: ty.clone(),
					use_cache: options.use_cache,
				});
			}
		}
	}

	Ok(inject_params)
}

/// Check if a type is `tonic::Request<T>` or `Request<T>` using AST inspection
///
/// This checks the type path segments rather than relying on string comparison,
/// providing more robust type detection that handles both qualified and
/// unqualified paths.
fn is_request_type(ty: &Type) -> bool {
	if let Type::Path(type_path) = ty {
		let segments = &type_path.path.segments;
		match segments.len() {
			// Unqualified: `Request<T>`
			1 => segments[0].ident == "Request",
			// Qualified: `tonic::Request<T>`
			2 => segments[0].ident == "tonic" && segments[1].ident == "Request",
			// Fully qualified with more segments
			_ => segments.last().is_some_and(|seg| seg.ident == "Request"),
		}
	} else {
		false
	}
}

/// Detect non-inject parameters (regular parameters)
fn detect_regular_params(inputs: &Punctuated<FnArg, Token![,]>) -> Vec<ParamInfo> {
	let mut params = Vec::new();

	for input in inputs {
		if let FnArg::Typed(pat_type) = input {
			// Skip parameters with #[inject] attribute
			if pat_type.attrs.iter().any(is_inject_attr) {
				continue;
			}

			params.push(ParamInfo {
				pat: pat_type.pat.clone(),
				ty: pat_type.ty.clone(),
			});
		} else if let FnArg::Receiver(_) = input {
			// Include &self parameters
			params.push(ParamInfo {
				pat: Box::new(Pat::Verbatim(quote! { self })),
				ty: Box::new(Type::Verbatim(quote! { &Self })),
			});
		}
	}

	params
}

/// Detect if the function has a `&self` receiver parameter using `FnArg::Receiver`
///
/// This uses the proper AST node type rather than relying on pattern matching
/// against `Pat::Verbatim`, providing more reliable self-parameter detection.
fn has_self_receiver(inputs: &Punctuated<FnArg, Token![,]>) -> bool {
	inputs
		.first()
		.is_some_and(|arg| matches!(arg, FnArg::Receiver(_)))
}

/// Strip `#[inject]` attributes from function parameters
fn strip_inject_attrs(inputs: &Punctuated<FnArg, Token![,]>) -> Vec<FnArg> {
	inputs
		.iter()
		.map(|arg| {
			if let FnArg::Typed(pat_type) = arg {
				let mut pat_type = pat_type.clone();
				pat_type.attrs.retain(|attr| !is_inject_attr(attr));
				FnArg::Typed(pat_type)
			} else {
				arg.clone()
			}
		})
		.collect()
}

/// Generate wrapper function with DI support
pub(crate) fn expand_grpc_handler(input: ItemFn) -> Result<TokenStream> {
	// Fixes #822: Validate that the function is async before proceeding
	if input.sig.asyncness.is_none() {
		return Err(Error::new_spanned(
			input.sig.fn_token,
			"#[grpc_handler] can only be applied to async functions",
		));
	}

	let inject_params = detect_inject_params(&input.sig.inputs)?;

	// If no #[inject] parameters, return the function as-is
	if inject_params.is_empty() {
		return Ok(quote! { #input });
	}

	let regular_params = detect_regular_params(&input.sig.inputs);

	// Original function name
	let original_fn_name = &input.sig.ident;

	// Fixes #823: Use less common prefix to avoid name collisions and use
	// original span for better error diagnostics
	let impl_fn_name = syn::Ident::new(
		&format!("__reinhardt_grpc_impl_{}", original_fn_name),
		original_fn_name.span(),
	);

	// Function visibility, attributes (excluding #[grpc_handler]), return type, etc.
	let vis = &input.vis;
	let fn_attrs: Vec<_> = input
		.attrs
		.iter()
		.filter(|attr| !attr.path().is_ident("grpc_handler"))
		.collect();
	let return_type = &input.sig.output;
	let asyncness = &input.sig.asyncness;
	let generics = &input.sig.generics;

	// Original function body
	let body = &input.block;

	// Strip #[inject] attributes from parameters for the impl function
	let impl_inputs = strip_inject_attrs(&input.sig.inputs);
	let impl_inputs = Punctuated::<FnArg, Token![,]>::from_iter(impl_inputs);

	// Generate DI context extraction
	// Use AST-based type inspection instead of string comparison
	let request_param = regular_params
		.iter()
		.find(|p| is_request_type(&p.ty))
		.ok_or_else(|| {
			Error::new(
				Span::call_site(),
				"#[grpc_handler] requires a tonic::Request<T> parameter",
			)
		})?;

	let request_pat = &request_param.pat;

	// Get dynamic crate paths
	let di_crate = get_reinhardt_di_crate();
	let grpc_crate = get_reinhardt_grpc_crate();

	// Generate DI extraction code
	// Fixes #820: Return generic error message, log details server-side
	let di_context_extraction = quote! {
		let __di_ctx = #request_pat
			.get_di_context::<::std::sync::Arc<#di_crate::InjectionContext>>()
			.ok_or_else(|| {
				::tracing::error!("DI context not found in request extensions");
				::tonic::Status::internal("Internal server error")
			})?;
	};

	// Generate compile-time type assertions for injected types
	// This ensures all #[inject] types implement Injectable at compile time
	// rather than failing at runtime with confusing errors
	// Use unique const names with function name prefix to avoid conflicts
	// when multiple #[grpc_handler] functions are in the same impl block
	let fn_name_str = original_fn_name.to_string();
	let type_assertions: Vec<_> = inject_params
		.iter()
		.enumerate()
		.map(|(idx, param)| {
			let ty = &param.ty;
			let const_name = syn::Ident::new(
				&format!("__ASSERT_{}_INJECTABLE_{}", fn_name_str.to_uppercase(), idx),
				Span::call_site(),
			);
			quote! {
				const #const_name: () = {
					// Compile-time assertion: #[inject] parameter type must implement Injectable
					fn __assert_injectable<__T: #di_crate::Injectable>() {}
					fn __check() { __assert_injectable::<#ty>() }
				};
			}
		})
		.collect();

	// Generate injection calls
	let injection_calls: Vec<_> = inject_params
		.iter()
		.map(|param| {
			let pat = &param.pat;
			let ty = &param.ty;
			let use_cache = param.use_cache;

			if use_cache {
				quote! {
					let #pat: #ty = #di_crate::Injected::<#ty>::resolve(&__di_ctx)
						.await
						.map_err(|e| {
							::tracing::error!("DI resolution failed for {}: {:?}", stringify!(#ty), e);
							::tonic::Status::internal("Internal server error")
						})?
						.into_inner();
				}
			} else {
				quote! {
					let #pat: #ty = #di_crate::Injected::<#ty>::resolve_uncached(&__di_ctx)
						.await
						.map_err(|e| {
							::tracing::error!("DI resolution failed for {}: {:?}", stringify!(#ty), e);
							::tonic::Status::internal("Internal server error")
						})?
						.into_inner();
				}
			}
		})
		.collect();

	// Check if this is a method using proper AST receiver detection
	let has_self = has_self_receiver(&input.sig.inputs);

	// Collect parameter names for the original function call (excluding self)
	let regular_args: Vec<_> = regular_params
		.iter()
		.skip(if has_self { 1 } else { 0 })
		.map(|p| &p.pat)
		.collect();
	let inject_args: Vec<_> = inject_params.iter().map(|param| &param.pat).collect();

	// Wrapper function inputs (only regular parameters, without #[inject])
	// Use has_self flag from proper receiver detection instead of Pat::Verbatim matching
	let wrapper_inputs: Vec<_> = regular_params
		.iter()
		.enumerate()
		.map(|(i, p)| {
			let pat = &p.pat;
			let ty = &p.ty;
			if i == 0 && has_self {
				// &self parameter detected via FnArg::Receiver
				quote! { &self }
			} else {
				quote! { #pat: #ty }
			}
		})
		.collect();
	let wrapper_inputs = Punctuated::<TokenStream, Token![,]>::from_iter(wrapper_inputs);

	// Generate the call to the impl function
	let impl_call = if has_self {
		quote! { self.#impl_fn_name(#(#regular_args,)* #(#inject_args),*).await }
	} else {
		quote! { #impl_fn_name(#(#regular_args,)* #(#inject_args),*).await }
	};

	// Generate the wrapper function
	let expanded = quote! {
		// Compile-time type assertions for injected dependencies
		#(#type_assertions)*

		// Original function (renamed to {name}_impl)
		#(#fn_attrs)*
		#asyncness fn #impl_fn_name #generics(#impl_inputs) #return_type #body

		// Wrapper function (keeps the original name)
		#(#fn_attrs)*
		#vis #asyncness fn #original_fn_name #generics(#wrapper_inputs) #return_type {
			use #grpc_crate::GrpcRequestExt;

			// Extract DI context
			#di_context_extraction

			// Resolve dependencies
			#(#injection_calls)*

			// Call original function
			#impl_call
		}
	};

	Ok(expanded)
}
