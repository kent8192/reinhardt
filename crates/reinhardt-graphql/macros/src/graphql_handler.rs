//! GraphQL handler macro implementation
//!
//! Provides the `#[graphql_handler]` attribute macro for GraphQL resolvers
//! that use dependency injection.

use crate::crate_paths::{get_reinhardt_di_crate, get_reinhardt_graphql_crate};
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

/// Parse `#[inject]` or `#[inject(cache = false)]` attributes
fn parse_inject_options(attrs: &[syn::Attribute]) -> InjectOptions {
	let mut options = InjectOptions {
		use_cache: true, // Default to caching enabled
	};

	for attr in attrs {
		if !is_inject_attr(attr) {
			continue;
		}

		// Try to parse as Meta::List: #[inject(cache = false)]
		if let syn::Meta::List(meta_list) = &attr.meta
			&& let Ok(nested) =
				meta_list.parse_args_with(Punctuated::<syn::Meta, Token![,]>::parse_terminated)
		{
			for meta in nested {
				if let syn::Meta::NameValue(nv) = meta
					&& nv.path.is_ident("cache")
					&& let syn::Expr::Lit(syn::ExprLit {
						lit: syn::Lit::Bool(lit_bool),
						..
					}) = &nv.value
				{
					options.use_cache = lit_bool.value;
				}
			}
		}
	}

	options
}

/// Detect parameters with `#[inject]` attribute
fn detect_inject_params(inputs: &Punctuated<FnArg, Token![,]>) -> Vec<InjectInfo> {
	let mut inject_params = Vec::new();

	for input in inputs {
		if let FnArg::Typed(PatType { attrs, pat, ty, .. }) = input {
			let has_inject = attrs.iter().any(is_inject_attr);

			if has_inject {
				let options = parse_inject_options(attrs);
				inject_params.push(InjectInfo {
					pat: pat.clone(),
					ty: ty.clone(),
					use_cache: options.use_cache,
				});
			}
		}
	}

	inject_params
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
pub(crate) fn expand_graphql_handler(input: ItemFn) -> Result<TokenStream> {
	let inject_params = detect_inject_params(&input.sig.inputs);

	// If no #[inject] parameters, return the function as-is
	if inject_params.is_empty() {
		return Ok(quote! { #input });
	}

	let regular_params = detect_regular_params(&input.sig.inputs);

	// Original function name
	let original_fn_name = &input.sig.ident;

	// Create new name for the original function
	let impl_fn_name = syn::Ident::new(&format!("{}_impl", original_fn_name), Span::call_site());

	// Function visibility, attributes (excluding #[graphql_handler]), return type, etc.
	let vis = &input.vis;
	let fn_attrs: Vec<_> = input
		.attrs
		.iter()
		.filter(|attr| !attr.path().is_ident("graphql_handler"))
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
	// We need to detect which parameter is async_graphql::Context<'_>
	let context_param = regular_params
		.iter()
		.find(|p| {
			if let Type::Reference(type_ref) = &*p.ty
				&& let Type::Path(type_path) = &*type_ref.elem
			{
				return type_path
					.path
					.segments
					.last()
					.map(|seg| seg.ident == "Context")
					.unwrap_or(false);
			}
			false
		})
		.ok_or_else(|| {
			Error::new(
				Span::call_site(),
				"#[graphql_handler] requires an async_graphql::Context parameter",
			)
		})?;

	let context_pat = &context_param.pat;

	// Get dynamic crate paths
	let di_crate = get_reinhardt_di_crate()?;
	let graphql_crate = get_reinhardt_graphql_crate()?;

	// Generate DI extraction code
	let di_context_extraction = quote! {
		let __di_ctx = #context_pat
			.get_di_context()
			.map_err(|e| ::async_graphql::Error::new(format!(
				"DI context not set. Ensure the schema was built with .data(injection_ctx): {:?}", e
			)))?;
	};

	// Generate injection calls
	let injection_calls: Vec<_> = inject_params
		.iter()
		.map(|param| {
			let pat = &param.pat;
			let ty = &param.ty;
			let use_cache = param.use_cache;

			if use_cache {
				quote! {
					let #pat: #ty = #di_crate::Injected::<#ty>::resolve(__di_ctx)
						.await
						.map_err(|e| ::async_graphql::Error::new(
							format!("Dependency injection failed for {}: {:?}", stringify!(#ty), e)
						))?
						.into_inner();
				}
			} else {
				quote! {
					let #pat: #ty = #di_crate::Injected::<#ty>::resolve_uncached(__di_ctx)
						.await
						.map_err(|e| ::async_graphql::Error::new(
							format!("Dependency injection failed for {}: {:?}", stringify!(#ty), e)
						))?
						.into_inner();
				}
			}
		})
		.collect();

	// Collect parameter names for the original function call
	let regular_args: Vec<_> = regular_params.iter().map(|p| &p.pat).collect();
	let inject_args: Vec<_> = inject_params.iter().map(|param| &param.pat).collect();

	// Wrapper function inputs (only regular parameters, without #[inject])
	let wrapper_inputs: Vec<_> = regular_params
		.iter()
		.enumerate()
		.map(|(i, p)| {
			let pat = &p.pat;
			let ty = &p.ty;
			if i == 0 && matches!(**pat, Pat::Verbatim(_)) {
				// &self parameter
				quote! { &self }
			} else {
				quote! { #pat: #ty }
			}
		})
		.collect();
	let wrapper_inputs = Punctuated::<TokenStream, Token![,]>::from_iter(wrapper_inputs);

	// Generate the wrapper function
	let expanded = quote! {
		// Original function (renamed to {name}_impl)
		#(#fn_attrs)*
		#asyncness fn #impl_fn_name #generics(#impl_inputs) #return_type #body

		// Wrapper function (keeps the original name)
		#(#fn_attrs)*
		#vis #asyncness fn #original_fn_name #generics(#wrapper_inputs) #return_type {
			use #graphql_crate::GraphQLContextExt;

			// Extract DI context
			#di_context_extraction

			// Resolve dependencies
			#(#injection_calls)*

			// Call original function
			#impl_fn_name(#(#regular_args,)* #(#inject_args),*).await
		}
	};

	Ok(expanded)
}
