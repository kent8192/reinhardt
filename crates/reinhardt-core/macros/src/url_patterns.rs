use proc_macro2::TokenStream;
use quote::quote;
use syn::{ItemFn, parse2};

/// Extract endpoint path expressions from a function body.
///
/// Scans tokens for `.endpoint(EXPR)` patterns and returns
/// the path expressions.
fn extract_endpoint_paths(func: &ItemFn) -> Vec<TokenStream> {
	let mut paths = Vec::new();
	let body_tokens: Vec<proc_macro2::TokenTree> = func
		.block
		.stmts
		.iter()
		.flat_map(|stmt| {
			let tokens: TokenStream = quote! { #stmt };
			tokens.into_iter().collect::<Vec<_>>()
		})
		.collect();

	let mut i = 0;
	while i < body_tokens.len() {
		// Look for: Punct('.') Ident("endpoint") Group(Parenthesized)
		if i + 2 < body_tokens.len()
			&& let proc_macro2::TokenTree::Punct(p) = &body_tokens[i]
			&& p.as_char() == '.'
			&& let proc_macro2::TokenTree::Ident(ident) = &body_tokens[i + 1]
			&& ident == "endpoint"
			&& let proc_macro2::TokenTree::Group(group) = &body_tokens[i + 2]
			&& group.delimiter() == proc_macro2::Delimiter::Parenthesis
		{
			paths.push(group.stream());
			i += 3;
			continue;
		}
		i += 1;
	}
	paths
}

/// Build a re-export statement for a URL resolver module from an endpoint path.
///
/// Given an endpoint path like `views::login`, generates:
/// `pub use super::views::__url_resolver_login::*;`
///
/// For absolute paths starting with `crate::` or `super::`, the `super::` prefix is omitted:
/// `pub use crate::views::__url_resolver_login::*;`
fn build_resolver_reexport(path: &TokenStream) -> TokenStream {
	let parsed: syn::Path = match syn::parse2(path.clone()) {
		Ok(p) => p,
		Err(_) => return quote! {},
	};

	if parsed.segments.is_empty() {
		return quote! {};
	}

	let last_segment = &parsed.segments.last().unwrap().ident;
	let resolver_mod = syn::Ident::new(
		&format!("__url_resolver_{last_segment}"),
		last_segment.span(),
	);

	let first_segment = parsed.segments.first().unwrap().ident.to_string();
	let is_absolute = first_segment == "crate" || first_segment == "super";

	let parent_segments: Vec<&syn::Ident> = parsed
		.segments
		.iter()
		.take(parsed.segments.len() - 1)
		.map(|s| &s.ident)
		.collect();

	if is_absolute {
		quote! {
			pub use #(#parent_segments ::)* #resolver_mod::*;
		}
	} else {
		quote! {
			pub use super:: #(#parent_segments ::)* #resolver_mod::*;
		}
	}
}

/// Build a path to a URL resolver metadata macro from an endpoint path.
///
/// Given `views::login`, generates:
/// `super::views::__url_resolver_login::__url_resolver_meta_login`
fn build_meta_reexport(path: &TokenStream) -> TokenStream {
	let parsed: syn::Path = match syn::parse2(path.clone()) {
		Ok(p) => p,
		Err(_) => return quote! {},
	};

	if parsed.segments.is_empty() {
		return quote! {};
	}

	let last_segment = &parsed.segments.last().unwrap().ident;
	let resolver_mod = syn::Ident::new(
		&format!("__url_resolver_{last_segment}"),
		last_segment.span(),
	);
	let meta_macro = syn::Ident::new(
		&format!("__url_resolver_meta_{last_segment}"),
		last_segment.span(),
	);

	let first_segment = parsed.segments.first().unwrap().ident.to_string();
	let is_absolute = first_segment == "crate" || first_segment == "super";

	let parent_segments: Vec<&syn::Ident> = parsed
		.segments
		.iter()
		.take(parsed.segments.len() - 1)
		.map(|s| &s.ident)
		.collect();

	if is_absolute {
		quote! {
			#(#parent_segments ::)* #resolver_mod :: #meta_macro
		}
	} else {
		quote! {
			super :: #(#parent_segments ::)* #resolver_mod :: #meta_macro
		}
	}
}

/// Implementation of the `#[url_patterns]` attribute macro.
///
/// Optionally accepts an app label string: `#[url_patterns("users")]`.
/// When provided, the returned router is wrapped with `.with_namespace("users")`
/// to enable per-app route name namespacing (Issue #3526).
pub(crate) fn url_patterns_impl(
	args: TokenStream,
	input: TokenStream,
) -> syn::Result<TokenStream> {
	let func: ItemFn = parse2(input)?;
	let endpoint_paths = extract_endpoint_paths(&func);

	let re_exports = endpoint_paths.iter().map(build_resolver_reexport);
	let meta_paths: Vec<TokenStream> = endpoint_paths.iter().map(build_meta_reexport).collect();

	// Parse optional app label: #[url_patterns("users")]
	let func_output = if !args.is_empty() {
		let app_label: syn::LitStr = syn::parse2(args)?;
		let fn_vis = &func.vis;
		let fn_attrs = &func.attrs;
		let fn_sig = &func.sig;
		let fn_block = &func.block;

		// Wrap the function to apply with_namespace
		quote! {
			#(#fn_attrs)*
			#fn_vis #fn_sig {
				let __router = (|| #fn_block)();
				__router.with_namespace(#app_label)
			}
		}
	} else {
		// No app label — emit function unchanged (backward compatible)
		quote! { #func }
	};

	Ok(quote! {
		#func_output

		#[doc(hidden)]
		pub mod url_resolvers {
			#(
				#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
				#re_exports
			)*

			/// Invoke a callback macro for each URL resolver in this app.
			/// Used by the `#[routes]` macro to build per-app resolver structs.
			#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
			macro_rules! __for_each_url_resolver {
				($callback:ident, $app:ident) => {
					#(
						#meta_paths ! ($callback, $app);
					)*
				};
			}
			#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
			pub(crate) use __for_each_url_resolver;
		}
	})
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn extract_single_endpoint() {
		let func: ItemFn = parse2(quote! {
			pub fn url_patterns() -> ServerRouter {
				ServerRouter::new()
					.endpoint(views::login)
			}
		})
		.unwrap();

		let paths = extract_endpoint_paths(&func);
		assert_eq!(paths.len(), 1);
		assert_eq!(paths[0].to_string(), "views :: login");
	}

	#[test]
	fn extract_multiple_endpoints() {
		let func: ItemFn = parse2(quote! {
			pub fn url_patterns() -> ServerRouter {
				ServerRouter::new()
					.endpoint(views::login)
					.endpoint(views::register)
					.endpoint(views::profile)
			}
		})
		.unwrap();

		let paths = extract_endpoint_paths(&func);
		assert_eq!(paths.len(), 3);
	}

	#[test]
	fn extract_no_endpoints() {
		let func: ItemFn = parse2(quote! {
			pub fn url_patterns() -> ServerRouter {
				ServerRouter::new()
			}
		})
		.unwrap();

		let paths = extract_endpoint_paths(&func);
		assert_eq!(paths.len(), 0);
	}

	#[test]
	fn extract_endpoints_mixed_with_other_calls() {
		let func: ItemFn = parse2(quote! {
			pub fn url_patterns() -> ServerRouter {
				ServerRouter::new()
					.with_middleware(auth_middleware)
					.endpoint(views::login)
					.endpoint(views::register)
			}
		})
		.unwrap();

		let paths = extract_endpoint_paths(&func);
		assert_eq!(paths.len(), 2);
	}

	#[test]
	fn build_reexport_relative_path() {
		let path: TokenStream = quote! { views::login };
		let result = build_resolver_reexport(&path);
		let expected = "pub use super :: views :: __url_resolver_login :: * ;";
		assert_eq!(result.to_string(), expected);
	}

	#[test]
	fn build_reexport_crate_path() {
		let path: TokenStream = quote! { crate::views::login };
		let result = build_resolver_reexport(&path);
		let expected = "pub use crate :: views :: __url_resolver_login :: * ;";
		assert_eq!(result.to_string(), expected);
	}

	#[test]
	fn build_reexport_super_path() {
		let path: TokenStream = quote! { super::views::login };
		let result = build_resolver_reexport(&path);
		let expected = "pub use super :: views :: __url_resolver_login :: * ;";
		assert_eq!(result.to_string(), expected);
	}

	#[test]
	fn build_reexport_deeply_nested_path() {
		let path: TokenStream = quote! { api::v1::views::login };
		let result = build_resolver_reexport(&path);
		let expected = "pub use super :: api :: v1 :: views :: __url_resolver_login :: * ;";
		assert_eq!(result.to_string(), expected);
	}

	// --- Meta reexport tests (Issue #3526) ---

	#[test]
	fn build_meta_reexport_relative_path() {
		let path: TokenStream = quote! { views::login };
		let result = build_meta_reexport(&path);
		let expected =
			"super :: views :: __url_resolver_login :: __url_resolver_meta_login";
		assert_eq!(result.to_string(), expected);
	}

	#[test]
	fn build_meta_reexport_crate_path() {
		let path: TokenStream = quote! { crate::views::login };
		let result = build_meta_reexport(&path);
		let expected =
			"crate :: views :: __url_resolver_login :: __url_resolver_meta_login";
		assert_eq!(result.to_string(), expected);
	}

	#[test]
	fn url_patterns_impl_generates_for_each_macro() {
		let input = quote! {
			pub fn url_patterns() -> ServerRouter {
				ServerRouter::new()
					.endpoint(views::login)
					.endpoint(views::register)
			}
		};

		let result = url_patterns_impl(quote! {}, input).unwrap();
		let output = result.to_string();

		assert!(
			output.contains("__for_each_url_resolver"),
			"missing __for_each_url_resolver macro"
		);
		assert!(
			output.contains("__url_resolver_meta_login"),
			"missing login meta path"
		);
		assert!(
			output.contains("__url_resolver_meta_register"),
			"missing register meta path"
		);
	}

	#[test]
	fn url_patterns_with_app_label_wraps_namespace() {
		let input = quote! {
			pub fn url_patterns() -> ServerRouter {
				ServerRouter::new()
					.endpoint(views::login)
			}
		};

		let result = url_patterns_impl(quote! { "users" }, input).unwrap();
		let output = result.to_string();

		assert!(
			output.contains("with_namespace"),
			"missing with_namespace call"
		);
	}

	#[test]
	fn url_patterns_without_app_label_no_namespace() {
		let input = quote! {
			pub fn url_patterns() -> ServerRouter {
				ServerRouter::new()
					.endpoint(views::login)
			}
		};

		let result = url_patterns_impl(quote! {}, input).unwrap();
		let output = result.to_string();

		assert!(
			!output.contains("with_namespace"),
			"should not have with_namespace without label"
		);
	}
}
