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

/// Implementation of the `#[url_patterns]` attribute macro.
pub(crate) fn url_patterns_impl(
	_args: TokenStream,
	input: TokenStream,
) -> syn::Result<TokenStream> {
	let func: ItemFn = parse2(input)?;
	let endpoint_paths = extract_endpoint_paths(&func);

	let re_exports = endpoint_paths.iter().map(|path| {
		quote! {
			pub use super::#path::__url_resolver::*;
		}
	});

	Ok(quote! {
		#func

		#[cfg(feature = "url-resolver")]
		#[doc(hidden)]
		pub mod url_resolvers {
			#(#re_exports)*
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
}
