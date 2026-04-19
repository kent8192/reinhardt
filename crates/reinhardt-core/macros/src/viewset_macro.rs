use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{ItemFn, parse2};

/// Extract basename string literal from a function body.
///
/// Scans tokens for `ModelViewSet::new("basename")` or `GenericViewSet::new("basename", ...)`
/// patterns and returns the first string literal found as the basename.
fn extract_basename(func: &ItemFn) -> Option<String> {
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
		// Look for: Ident("ModelViewSet" | "GenericViewSet") Punct(':') Punct(':')
		if i + 2 < body_tokens.len()
			&& let proc_macro2::TokenTree::Ident(type_ident) = &body_tokens[i]
			&& (type_ident == "ModelViewSet" || type_ident == "GenericViewSet")
			&& let proc_macro2::TokenTree::Punct(p1) = &body_tokens[i + 1]
			&& p1.as_char() == ':'
			&& let proc_macro2::TokenTree::Punct(p2) = &body_tokens[i + 2]
			&& p2.as_char() == ':'
		{
			// Check for `new` ident and parenthesized group
			if i + 4 < body_tokens.len()
				&& let proc_macro2::TokenTree::Ident(new_ident) = &body_tokens[i + 3]
				&& new_ident == "new"
				&& let proc_macro2::TokenTree::Group(group) = &body_tokens[i + 4]
				&& group.delimiter() == proc_macro2::Delimiter::Parenthesis
			{
				// Extract first string literal from the group
				for tt in group.stream() {
					if let proc_macro2::TokenTree::Literal(lit) = tt {
						let lit_str = lit.to_string();
						if lit_str.starts_with('"') && lit_str.ends_with('"') {
							return Some(lit_str[1..lit_str.len() - 1].to_string());
						}
					}
				}
			}
		}
		i += 1;
	}
	None
}

/// Convert a basename like "snippet" to PascalCase: "Snippet".
/// Handles underscored names: "auth_user" → "AuthUser".
fn to_pascal_case(s: &str) -> String {
	let mut result = String::new();
	for segment in s.split('_') {
		let mut chars = segment.chars();
		if let Some(first) = chars.next() {
			result.push(first.to_ascii_uppercase());
			result.extend(chars);
		}
	}
	result
}

/// Generate URL resolver modules for a ViewSet basename.
///
/// For basename "snippet", generates:
/// - `__url_resolver_snippet_list` with trait `ResolveSnippetList`
/// - `__url_resolver_snippet_detail` with trait `ResolveSnippetDetail`
///
/// These modules are emitted at the same level as the annotated function.
/// Because `mod` items are invalid inside `impl` blocks, `#[viewset]` must
/// be applied to a free (module-level) function, not a method.
fn generate_viewset_resolver_tokens(basename: &str) -> TokenStream {
	let pascal = to_pascal_case(basename);

	let list_mod_ident = syn::Ident::new(
		&format!("__url_resolver_{basename}_list"),
		Span::call_site(),
	);
	let detail_mod_ident = syn::Ident::new(
		&format!("__url_resolver_{basename}_detail"),
		Span::call_site(),
	);

	let list_trait_ident = syn::Ident::new(&format!("Resolve{pascal}List"), Span::call_site());
	let detail_trait_ident = syn::Ident::new(&format!("Resolve{pascal}Detail"), Span::call_site());

	let list_method_ident = syn::Ident::new(&format!("{basename}_list"), Span::call_site());
	let detail_method_ident = syn::Ident::new(&format!("{basename}_detail"), Span::call_site());

	let list_route_name = format!("{basename}-list");
	let detail_route_name = format!("{basename}-detail");

	let list_doc = format!("Resolve URL for route `{list_route_name}`.");
	let detail_doc = format!("Resolve URL for route `{detail_route_name}`.");

	let reinhardt_crate = crate::crate_paths::get_reinhardt_crate();

	quote! {
		#[doc(hidden)]
		pub mod #list_mod_ident {
			#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
			#[doc = #list_doc]
			pub trait #list_trait_ident: #reinhardt_crate::UrlResolver {
				#[doc = #list_doc]
				fn #list_method_ident(&self) -> String {
					self.resolve_url(#list_route_name, &[])
				}
			}
			#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
			impl<T: #reinhardt_crate::UrlResolver> #list_trait_ident for T {}
		}
		#[doc(hidden)]
		pub use #list_mod_ident::*;

		#[doc(hidden)]
		pub mod #detail_mod_ident {
			#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
			#[doc = #detail_doc]
			pub trait #detail_trait_ident: #reinhardt_crate::UrlResolver {
				#[doc = #detail_doc]
				fn #detail_method_ident(&self, id: &str) -> String {
					self.resolve_url(#detail_route_name, &[("id", id)])
				}
			}
			#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
			impl<T: #reinhardt_crate::UrlResolver> #detail_trait_ident for T {}
		}
		#[doc(hidden)]
		pub use #detail_mod_ident::*;
	}
}

/// Implementation of the `#[viewset]` attribute macro.
pub(crate) fn viewset_macro_impl(
	_args: TokenStream,
	input: TokenStream,
) -> syn::Result<TokenStream> {
	let func: ItemFn = parse2(input)?;
	let fn_name = &func.sig.ident;

	let basename = extract_basename(&func).ok_or_else(|| {
		syn::Error::new_spanned(
			&func.sig.ident,
			"#[viewset] could not extract basename. \
			 Expected ModelViewSet::new(\"basename\") or \
			 GenericViewSet::new(\"basename\", ...) in the function body.",
		)
	})?;

	let resolver_tokens = generate_viewset_resolver_tokens(&basename);

	// Generate a well-known bundle module that #[url_patterns] can reference
	// by function name (which it CAN see from tokens).
	let bundle_mod_ident =
		syn::Ident::new(&format!("__viewset_resolvers_{fn_name}"), Span::call_site());
	let list_mod_ident = syn::Ident::new(
		&format!("__url_resolver_{basename}_list"),
		Span::call_site(),
	);
	let detail_mod_ident = syn::Ident::new(
		&format!("__url_resolver_{basename}_detail"),
		Span::call_site(),
	);

	Ok(quote! {
		#func
		#resolver_tokens

		#[doc(hidden)]
		pub mod #bundle_mod_ident {
			#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
			pub use super::#list_mod_ident::*;
			#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
			pub use super::#detail_mod_ident::*;
		}
	})
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn extract_basename_model_viewset() {
		let func: ItemFn = parse2(quote! {
			pub fn viewset() -> ModelViewSet<Snippet, SnippetSerializer> {
				ModelViewSet::new("snippet")
					.with_pagination(PaginationConfig::page_number(10, Some(100)))
			}
		})
		.unwrap();

		assert_eq!(extract_basename(&func), Some("snippet".to_string()));
	}

	#[test]
	fn extract_basename_generic_viewset() {
		let func: ItemFn = parse2(quote! {
			pub fn viewset() -> GenericViewSet<User> {
				GenericViewSet::new("user", ())
			}
		})
		.unwrap();

		assert_eq!(extract_basename(&func), Some("user".to_string()));
	}

	#[test]
	fn extract_basename_not_found() {
		let func: ItemFn = parse2(quote! {
			pub fn viewset() -> ServerRouter {
				ServerRouter::new()
			}
		})
		.unwrap();

		assert_eq!(extract_basename(&func), None);
	}

	#[test]
	fn to_pascal_case_single() {
		assert_eq!(to_pascal_case("snippet"), "Snippet");
	}

	#[test]
	fn to_pascal_case_multi() {
		assert_eq!(to_pascal_case("auth_user"), "AuthUser");
	}

	#[test]
	fn generate_resolver_tokens_contains_expected_identifiers() {
		let tokens = generate_viewset_resolver_tokens("snippet");
		let output = tokens.to_string();
		assert!(output.contains("__url_resolver_snippet_list"));
		assert!(output.contains("__url_resolver_snippet_detail"));
		assert!(output.contains("ResolveSnippetList"));
		assert!(output.contains("ResolveSnippetDetail"));
		assert!(output.contains("snippet_list"));
		assert!(output.contains("snippet_detail"));
	}
}
