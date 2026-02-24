//! Implementation of the `#[injectable_factory]` macro

use crate::crate_paths::get_reinhardt_di_crate;
use crate::utils::{extract_scope_from_args, is_inject_attr};
use proc_macro2::TokenStream;
use quote::quote;
use syn::{FnArg, ItemFn, Pat, PatType, Result};

/// Implementation of the `#[injectable_factory]` attribute macro
///
/// This macro:
/// 1. Extracts the return type of the factory function
/// 2. Analyzes parameters marked with `#[inject]`
/// 3. Generates a wrapper that resolves dependencies
/// 4. Registers the factory with the global registry using inventory
pub(crate) fn injectable_factory_impl(args: TokenStream, input: ItemFn) -> Result<TokenStream> {
	let fn_name = &input.sig.ident;
	let fn_vis = &input.vis;
	let fn_block = &input.block;
	let fn_attrs = &input.attrs;

	// Extract scope from macro arguments
	let scope = extract_scope_from_args(args)?;
	let scope_tokens = scope.into_tokens();

	// Validate that the function is async
	if input.sig.asyncness.is_none() {
		return Err(syn::Error::new_spanned(
			&input.sig,
			"#[injectable_factory] can only be applied to async functions",
		));
	}

	// Extract return type
	let return_type = match &input.sig.output {
		syn::ReturnType::Type(_, ty) => ty.as_ref().clone(),
		syn::ReturnType::Default => {
			return Err(syn::Error::new_spanned(
				&input.sig,
				"#[injectable_factory] functions must have an explicit return type",
			));
		}
	};

	// Analyze function parameters
	let mut inject_params = Vec::new();
	let mut regular_params = Vec::new();

	for arg in &input.sig.inputs {
		if let FnArg::Typed(PatType { attrs, pat, ty, .. }) = arg {
			let has_inject = attrs.iter().any(is_inject_attr);

			if has_inject {
				inject_params.push((pat.clone(), ty.clone()));
			} else {
				regular_params.push((pat.clone(), ty.clone()));
			}
		}
	}

	// Reject non-inject parameters with a clear compile error.
	// The generated wrapper function only receives an InjectionContext, so non-inject
	// parameters would be undefined in the generated code.
	if !regular_params.is_empty() {
		return Err(syn::Error::new_spanned(
			&input.sig,
			"#[injectable_factory] functions must have all parameters marked with #[inject]. \
			 Non-inject parameters are not supported because the generated wrapper function \
			 only receives an InjectionContext.",
		));
	}

	// Generate dependency resolution code
	let inject_resolutions: Vec<_> = inject_params
		.iter()
		.map(|(pat, ty)| {
			quote! {
				let #pat: #ty = ctx.resolve::<#ty>().await?;
			}
		})
		.collect();

	// Generate parameter names for the original function call
	let inject_param_names: Vec<_> = inject_params
		.iter()
		.map(|(pat, _)| {
			// Extract the identifier from the pattern
			if let Pat::Ident(pat_ident) = pat.as_ref() {
				let ident = &pat_ident.ident;
				quote! { #ident }
			} else {
				quote! { #pat }
			}
		})
		.collect();

	let regular_param_names: Vec<_> = regular_params
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

	// Generate the original function with a hygienic internal name.
	// Uses double-underscore `__reinhardt_` prefix to avoid collisions with user-defined names.
	let original_fn_name =
		syn::Ident::new(&format!("__reinhardt_{}_impl", fn_name), fn_name.span());
	let original_params: Vec<_> = inject_params
		.iter()
		.chain(regular_params.iter())
		.map(|(pat, ty)| quote! { #pat: #ty })
		.collect();

	// Get dynamic crate path
	let di_crate = get_reinhardt_di_crate();

	// Generate type name for registration
	let type_name = quote! { #return_type }.to_string();

	// Generate the expanded code
	let expanded = quote! {
		// Original implementation function (private)
		async fn #original_fn_name(#(#original_params),*) -> #return_type {
			#fn_block
		}

		// Public wrapper factory function
		#(#fn_attrs)*
		#fn_vis async fn #fn_name(
			ctx: ::std::sync::Arc<#di_crate::InjectionContext>,
		) -> #di_crate::DiResult<#return_type> {
			// Resolve #[inject] dependencies
			#(#inject_resolutions)*

			// Call the original function
			let result = #original_fn_name(#(#inject_param_names,)* #(#regular_param_names),*).await;
			Ok(result)
		}

		// Register with inventory
		#di_crate::inventory::submit! {
			#di_crate::DependencyRegistration::new::<#return_type, _, _>(
				#type_name,
				#scope_tokens,
				#fn_name
			)
		}
	};

	Ok(expanded)
}
