//! Injectable function macro for Factory/Provider pattern
//!
//! Provides `#[injectable]` attribute macro that generates `Injectable` trait
//! implementation for the return type of a function, enabling the function
//! to be used as a factory/provider for dependency injection.

use crate::crate_paths::{get_async_trait_crate, get_reinhardt_di_crate};
use crate::injectable_common::{InjectionScope, is_inject_attr, parse_inject_options};
use proc_macro2::TokenStream;
use quote::quote;
use syn::{FnArg, Ident, ItemFn, Pat, PatType, Result, ReturnType, Type};

/// Information about an `#[inject]` parameter
struct InjectParamInfo {
	name: Ident,
	ty: Type,
	use_cache: bool,
	scope: InjectionScope,
}

/// Implementation of the `#[injectable]` attribute macro
///
/// Transforms a factory/provider function into an `Injectable` trait implementation
/// for its return type. This enables the function's return type to be automatically
/// resolved from the DI container.
///
/// # Async Support
///
/// Both sync and async functions are supported.
pub(crate) fn injectable_fn_impl(_args: TokenStream, input: ItemFn) -> Result<TokenStream> {
	let fn_name = &input.sig.ident;
	let is_async = input.sig.asyncness.is_some();

	// Get return type
	let return_type = match &input.sig.output {
		ReturnType::Type(_, ty) => (**ty).clone(),
		ReturnType::Default => {
			return Err(syn::Error::new_spanned(
				&input.sig,
				"#[injectable] function must have a return type",
			));
		}
	};

	// Collect #[inject] parameters
	let mut inject_params = Vec::new();
	let mut non_inject_params = Vec::new();

	for arg in &input.sig.inputs {
		if let FnArg::Typed(PatType { attrs, pat, ty, .. }) = arg {
			let has_inject = attrs.iter().any(is_inject_attr);

			if has_inject {
				let name = match &**pat {
					Pat::Ident(pat_ident) => pat_ident.ident.clone(),
					_ => {
						return Err(syn::Error::new_spanned(
							pat,
							"#[inject] parameter must be a simple identifier",
						));
					}
				};

				let options = parse_inject_options(attrs);
				inject_params.push(InjectParamInfo {
					name,
					ty: (**ty).clone(),
					use_cache: options.use_cache,
					scope: options.scope,
				});
			} else {
				// Non-inject parameters are not supported for #[injectable] functions
				non_inject_params.push(arg.clone());
			}
		}
	}

	// Validate: #[injectable] functions should only have #[inject] parameters
	if !non_inject_params.is_empty() {
		return Err(syn::Error::new_spanned(
			&input.sig,
			"#[injectable] functions can only have #[inject] parameters",
		));
	}

	// Generate implementation function name
	let impl_fn_name = Ident::new(&format!("{}_impl", fn_name), fn_name.span());

	// Clone the function and rename it
	let mut impl_fn = input.clone();
	impl_fn.sig.ident = impl_fn_name.clone();
	impl_fn.vis = syn::Visibility::Inherited; // Make private

	// Remove #[inject] attributes from parameters
	for arg in impl_fn.sig.inputs.iter_mut() {
		if let FnArg::Typed(pat_type) = arg {
			pat_type.attrs.retain(|attr| !is_inject_attr(attr));
		}
	}

	// Get dynamic crate paths
	let di_crate = get_reinhardt_di_crate();
	// Fixes #791: Use dynamic resolution instead of hardcoded ::async_trait
	let async_trait = get_async_trait_crate();

	// Generate resolve statements with scope support
	let resolve_stmts: Vec<_> = inject_params
		.iter()
		.map(|param| {
			let name = &param.name;
			let ty = &param.ty;
			let use_cache = param.use_cache;

			match param.scope {
				InjectionScope::Singleton => {
					quote! {
						let #name: #ty = {
							// Check singleton cache first
							if let Some(cached) = __di_ctx.singleton_scope().get::<#ty>() {
								(*cached).clone()
							} else {
								let __injected = if #use_cache {
									#di_crate::Injected::<#ty>::resolve(__di_ctx).await
								} else {
									#di_crate::Injected::<#ty>::resolve_uncached(__di_ctx).await
								}
								.map_err(|e| {
									tracing::debug!(
										dependency_type = stringify!(#ty),
										"injectable function dependency resolution failed"
									);
									e
								})?;
								let value = (*__injected).clone();
								__di_ctx.singleton_scope().set(value.clone());
								value
							}
						};
					}
				}
				InjectionScope::Request => {
					quote! {
						let #name: #ty = {
							let __injected = if #use_cache {
								#di_crate::Injected::<#ty>::resolve(__di_ctx).await
							} else {
								#di_crate::Injected::<#ty>::resolve_uncached(__di_ctx).await
							}
							.map_err(|e| {
								tracing::debug!(
									dependency_type = stringify!(#ty),
									"injectable function dependency resolution failed"
								);
								e
							})?;
							(*__injected).clone()
						};
					}
				}
			}
		})
		.collect();

	// Generate call arguments
	let call_args: Vec<_> = inject_params.iter().map(|param| &param.name).collect();

	// Generate function call (sync or async)
	let fn_call = if is_async {
		quote! { #impl_fn_name(#(#call_args),*).await }
	} else {
		quote! { #impl_fn_name(#(#call_args),*) }
	};

	// Generate the expanded code with override support
	let expanded = quote! {
		// Original function implementation (renamed, private)
		#impl_fn

		/// Returns the function pointer for this injectable function.
		///
		/// This function is used to obtain the function pointer address
		/// for use with `InjectionContext::dependency()` override API.
		///
		/// # Examples
		///
		/// ```rust,no_run
		/// # use reinhardt_di::{InjectionContext, SingletonScope};
		/// # use std::sync::Arc;
		/// # let singleton = Arc::new(SingletonScope::new());
		/// # let ctx = InjectionContext::builder(singleton).build();
		/// # fn get_database() -> String { String::new() }
		/// # let mock_value = String::from("mock");
		/// ctx.dependency(get_database).override_with(mock_value);
		/// ```
		#[allow(dead_code)]
		pub fn #fn_name() -> #return_type {
			panic!(
				"This function should not be called directly. \
				Use Injectable::inject() or InjectionContext::dependency() instead."
			)
		}

		// Injectable trait implementation for the return type
		#[#async_trait::async_trait]
		impl #di_crate::Injectable for #return_type {
			async fn inject(__di_ctx: &#di_crate::InjectionContext)
				-> #di_crate::DiResult<Self>
			{
				// Check for override first
				let __func_ptr = #fn_name as usize;
				if let Some(__override_value) = __di_ctx.get_override::<Self>(__func_ptr) {
					return Ok(__override_value);
				}

				// Normal dependency resolution
				#(#resolve_stmts)*
				Ok(#fn_call)
			}
		}
	};

	Ok(expanded)
}

#[cfg(test)]
mod tests {
	use super::*;
	use syn::parse_quote;

	#[test]
	fn test_injectable_fn_simple() {
		let input: ItemFn = parse_quote! {
			fn create_service(
				#[inject] db: Database,
			) -> MyService {
				MyService { db }
			}
		};

		let result = injectable_fn_impl(quote!(), input);
		let output = result.unwrap().to_string();
		assert!(output.contains("Injectable"));
		assert!(output.contains("inject"));
		assert!(output.contains("MyService"));
		assert!(output.contains("create_service_impl"));
		// Check override support is generated
		assert!(output.contains("get_override"));
		assert!(output.contains("__func_ptr"));
	}

	#[test]
	fn test_injectable_fn_async() {
		let input: ItemFn = parse_quote! {
			async fn get_config() -> Config {
				Config::load().await
			}
		};

		let result = injectable_fn_impl(quote!(), input);
		let output = result.unwrap().to_string();
		assert!(output.contains("Injectable"));
		assert!(output.contains("Config"));
		assert!(output.contains("await"));
	}

	#[test]
	fn test_injectable_fn_no_return_type_error() {
		let input: ItemFn = parse_quote! {
			fn bad_function() {
				// No return type
			}
		};

		let result = injectable_fn_impl(quote!(), input);
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert!(err.to_string().contains("return type"));
	}

	#[test]
	fn test_injectable_fn_non_inject_param_error() {
		let input: ItemFn = parse_quote! {
			fn bad_function(regular_param: String) -> MyService {
				MyService {}
			}
		};

		let result = injectable_fn_impl(quote!(), input);
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert!(err.to_string().contains("#[inject]"));
	}

	#[test]
	fn test_injectable_fn_with_scope_singleton() {
		let input: ItemFn = parse_quote! {
			fn create_service(
				#[inject(scope = Singleton)] db: Database,
			) -> MyService {
				MyService { db }
			}
		};

		let result = injectable_fn_impl(quote!(), input);
		let output = result.unwrap().to_string();
		assert!(output.contains("Injectable"));
		assert!(output.contains("singleton_scope"));
	}

	#[test]
	fn test_injectable_fn_with_cache_and_scope() {
		let input: ItemFn = parse_quote! {
			fn create_service(
				#[inject(cache = false, scope = Singleton)] db: Database,
			) -> MyService {
				MyService { db }
			}
		};

		let result = injectable_fn_impl(quote!(), input);
		let output = result.unwrap().to_string();
		assert!(output.contains("Injectable"));
		assert!(output.contains("singleton_scope"));
	}
}
