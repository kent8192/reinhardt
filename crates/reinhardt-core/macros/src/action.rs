//! action macro implementation

use crate::crate_paths::get_reinhardt_di_crate;
use crate::injectable_common::{
	detect_inject_params, generate_di_context_extraction, generate_injection_calls,
	strip_inject_attrs,
};
use proc_macro2::TokenStream;
use quote::quote;
use syn::{
	Expr, ExprLit, FnArg, ItemFn, Lit, Meta, Result, Token, parse::Parser, punctuated::Punctuated,
};

/// Implementation of the `action` procedural macro
///
/// This function is used internally by the `#[action]` attribute macro.
/// Users should not call this function directly.
pub(crate) fn action_impl(args: TokenStream, input: ItemFn) -> Result<TokenStream> {
	let mut methods = Vec::new();
	let mut detail = false;
	let mut _url_path: Option<String> = None;
	let mut _url_name: Option<String> = None;
	let mut use_inject = false;
	let mut methods_lit = None;
	let mut has_methods = false;
	let mut has_detail = false;

	// Parse arguments
	let meta_list = Punctuated::<Meta, Token![,]>::parse_terminated.parse2(args)?;

	for meta in meta_list {
		match meta {
			Meta::NameValue(nv) => {
				if nv.path.is_ident("methods") {
					has_methods = true;
					if let Expr::Lit(ExprLit {
						lit: Lit::Str(lit), ..
					}) = &nv.value
					{
						methods_lit = Some(lit.clone());
						let methods_str = lit.value();
						let methods_str = methods_str.trim_matches(|c| c == '[' || c == ']');

						for method in methods_str.split(',') {
							let method = method.trim().trim_matches('"');
							if !method.is_empty() {
								methods.push(method.to_string());
							}
						}
					} else {
						return Err(syn::Error::new_spanned(
							&nv.value,
							"methods parameter must be a string literal",
						));
					}
				} else if nv.path.is_ident("detail") {
					has_detail = true;
					if let Expr::Lit(ExprLit {
						lit: Lit::Bool(lit),
						..
					}) = &nv.value
					{
						detail = lit.value;
					} else {
						return Err(syn::Error::new_spanned(
							&nv.value,
							"detail parameter must be a boolean literal (true or false)",
						));
					}
				} else if nv.path.is_ident("url_path") {
					if let Expr::Lit(ExprLit {
						lit: Lit::Str(lit), ..
					}) = &nv.value
					{
						let path = lit.value();
						// Validate url_path format
						if path.contains(' ') {
							return Err(syn::Error::new_spanned(
								lit,
								"url_path cannot contain spaces",
							));
						}
						if !path.starts_with('/') {
							return Err(syn::Error::new_spanned(
								lit,
								"url_path must start with '/'",
							));
						}
						_url_path = Some(path);
					} else {
						return Err(syn::Error::new_spanned(
							&nv.value,
							"url_path parameter must be a string literal",
						));
					}
				} else if nv.path.is_ident("url_name") {
					if let Expr::Lit(ExprLit {
						lit: Lit::Str(lit), ..
					}) = &nv.value
					{
						_url_name = Some(lit.value());
					} else {
						return Err(syn::Error::new_spanned(
							&nv.value,
							"url_name parameter must be a string literal",
						));
					}
				} else if nv.path.is_ident("use_inject") {
					if let Expr::Lit(ExprLit {
						lit: Lit::Bool(lit),
						..
					}) = &nv.value
					{
						use_inject = lit.value;
					} else {
						return Err(syn::Error::new_spanned(
							&nv.value,
							"use_inject parameter must be a boolean literal (true or false)",
						));
					}
				}
			}
			Meta::Path(path) => {
				if path.is_ident("methods") {
					return Err(syn::Error::new_spanned(
						path,
						"methods parameter requires a value: methods = \"GET,POST\"",
					));
				} else if path.is_ident("detail") {
					return Err(syn::Error::new_spanned(
						path,
						"detail parameter requires a value: detail = true or detail = false",
					));
				}
			}
			_ => {}
		}
	}

	// Validate required parameters
	if !has_methods {
		return Err(syn::Error::new(
			proc_macro2::Span::call_site(),
			"action macro requires 'methods' parameter",
		));
	}

	if !has_detail {
		return Err(syn::Error::new(
			proc_macro2::Span::call_site(),
			"action macro requires 'detail' parameter",
		));
	}

	// Default to GET if no methods specified
	if methods.is_empty() {
		methods.push("GET".to_string());
	}

	// Validate HTTP methods
	const VALID_METHODS: &[&str] = &["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"];
	for method in &methods {
		let method_upper = method.to_uppercase();
		if !VALID_METHODS.contains(&method_upper.as_str()) {
			let error_msg = format!(
				"Invalid HTTP method '{}'. Valid methods are: GET, POST, PUT, PATCH, DELETE, HEAD, OPTIONS",
				method
			);
			return Err(syn::Error::new_spanned(
				methods_lit.as_ref().unwrap(),
				error_msg,
			));
		}
	}

	let fn_name = &input.sig.ident;
	let fn_block = &input.block;
	let fn_inputs = &input.sig.inputs;
	let fn_output = &input.sig.output;
	let fn_vis = &input.vis;
	let fn_attrs = &input.attrs;
	let asyncness = &input.sig.asyncness;
	let generics = &input.sig.generics;
	let where_clause = &input.sig.generics.where_clause;

	// Generate metadata
	let detail_flag = detail;
	let method_list = methods.join(", ");

	// Detect #[inject] parameters
	let inject_params = detect_inject_params(fn_inputs);

	// Validate: error if #[inject] is used when use_inject = false
	if !use_inject && !inject_params.is_empty() {
		return Err(syn::Error::new_spanned(
			&inject_params[0].pat,
			"#[inject] attribute requires use_inject = true option",
		));
	}

	// Generate wrapper function for DI support
	if use_inject && !inject_params.is_empty() {
		let original_fn_name = quote::format_ident!("{}_original", fn_name);

		// Original function (with #[inject] attributes stripped)
		let stripped_inputs = strip_inject_attrs(fn_inputs);
		let stripped_inputs = Punctuated::<FnArg, Token![,]>::from_iter(stripped_inputs);

		// DI context extraction code
		let request_ident = syn::Ident::new("request", proc_macro2::Span::call_site());
		let di_extraction = generate_di_context_extraction(&request_ident);

		// Injection calls code
		let injection_calls = generate_injection_calls(&inject_params);

		// Argument list
		let inject_args: Vec<_> = inject_params.iter().map(|p| &p.pat).collect();
		let regular_args: Vec<_> = stripped_inputs
			.iter()
			.filter_map(|arg| {
				if let FnArg::Typed(pat_type) = arg {
					Some(&pat_type.pat)
				} else {
					None
				}
			})
			.collect();

		let di_crate = get_reinhardt_di_crate();

		Ok(quote! {
			// Original function (renamed, private)
			#asyncness fn #original_fn_name #generics (#stripped_inputs) #fn_output #where_clause {
				#fn_block
			}

			// Wrapper function (with DI support)
			#(#fn_attrs)*
			#[doc = "Custom action with DI support"]
			#[doc = concat!("Methods: ", #method_list)]
			#[doc = concat!("Detail: ", stringify!(#detail_flag))]
			#fn_vis #asyncness fn #fn_name(request: ::Request) #fn_output {
				// DI context extraction
				#di_extraction

				// Execute handler within resolve context scope
				#di_crate::resolve_context::RESOLVE_CTX.scope(__resolve_ctx, async {
					// Dependency resolution
					#(#injection_calls)*

					// Call original function
					#original_fn_name(#(#regular_args,)* #(#inject_args),*).await
				}).await
			}
		})
	} else {
		// Without DI, use conventional approach
		Ok(quote! {
			#(#fn_attrs)*
			#[doc = "Custom action"]
			#[doc = concat!("Methods: ", #method_list)]
			#[doc = concat!("Detail: ", stringify!(#detail_flag))]
			#fn_vis #asyncness fn #fn_name #generics (#fn_inputs) #fn_output #where_clause {
				#fn_block
			}
		})
	}
}
