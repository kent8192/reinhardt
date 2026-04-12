//! Implementation of the `#[injectable_factory]` macro

use crate::crate_paths::get_reinhardt_di_crate;
use crate::utils::{extract_depends_inner_type, extract_scope_from_args, is_inject_attr};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
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

	// Get dynamic crate path (needed by inject_resolutions below)
	let di_crate = get_reinhardt_di_crate();

	// Generate dependency resolution code.
	// `ctx.resolve::<T>()` returns `DiResult<Arc<T>>`, so we must handle two cases:
	// - Parameter type is `Depends<T>`: resolve via `Depends::resolve()` with caching
	// - Parameter type is `T` (non-Depends): resolve `T`, then clone out of the `Arc`
	let inject_resolutions: Vec<_> = inject_params
		.iter()
		.map(|(pat, ty)| {
			if let Some(inner_ty) = extract_depends_inner_type(ty) {
				// Parameter is Depends<T>: resolve via registry only (no Injectable bound needed).
				// Factory-produced types may not implement Injectable.
				quote! {
					let #pat: #ty = #di_crate::Depends::<#inner_ty>::resolve_from_registry(&*ctx, true).await?;
				}
			} else {
				// Parameter is T: resolve T, unwrap Arc<T> via clone
				quote! {
					let #pat: #ty = {
						let __arc = ctx.resolve::<#ty>().await?;
						(*__arc).clone()
					};
				}
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

	// Generate type name for registration
	let type_name = quote! { #return_type }.to_string();

	// Generate registration function name for const-safe inventory submission
	let register_fn_name = format_ident!("__reinhardt_register_{}", fn_name);

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
			#di_crate::with_cycle_detection_scope(async {
				// Set task-local resolve context before resolving dependencies.
				// Inherit root from outer scope (for nested factory calls),
				// or default to ctx itself at the top level.
				// This must be established first so that Depends::resolve() and
				// any Injectable::inject() impl can use get_di_context().
				let __resolve_ctx = #di_crate::resolve_context::ResolveContext {
					root: #di_crate::resolve_context::RESOLVE_CTX
						.try_with(|__outer| ::std::sync::Arc::clone(&__outer.root))
						.unwrap_or_else(|_| ::std::sync::Arc::clone(&ctx)),
					current: ::std::sync::Arc::clone(&ctx),
				};

				let result = #di_crate::resolve_context::RESOLVE_CTX
					.scope(__resolve_ctx, async {
						// Resolve #[inject] dependencies
						#(#inject_resolutions)*

						Ok(#original_fn_name(#(#inject_param_names,)* #(#regular_param_names),*).await)
					})
					.await?;
				Ok(result)
			}).await
		}

		// Registration function for const-safe inventory::submit
		fn #register_fn_name(registry: &#di_crate::DependencyRegistry) {
			registry.register_async::<#return_type, _, _>(#scope_tokens, #fn_name);
			registry.register_type_name(
				::std::any::TypeId::of::<#return_type>(),
				#type_name,
			);
			registry.register_qualified_type_name(
				::std::any::TypeId::of::<#return_type>(),
				::std::any::type_name::<#return_type>(),
			);
		}

		#di_crate::inventory::submit! {
			#di_crate::DependencyRegistration::new::<#return_type>(
				#type_name,
				#scope_tokens,
				#register_fn_name
			)
		}

	};

	Ok(expanded)
}
