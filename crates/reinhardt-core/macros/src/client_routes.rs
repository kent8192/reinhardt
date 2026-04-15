//! Implementation of the `client_routes!` proc macro.
//!
//! Generates runtime `named_route()` / `named_route_path()` calls and
//! compile-time metadata macros for per-app client URL resolver structs,
//! mirroring the server-side pattern used by `#[get]`, `#[post]`, etc.

use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{Expr, Ident, LitStr, Result, Token};

/// Extract parameter names from a URL pattern string (e.g., `/users/{id}/` → `["id"]`).
fn extract_url_params(pattern: &str) -> Vec<String> {
	let mut params = Vec::new();
	let mut chars = pattern.chars().peekable();
	while let Some(ch) = chars.next() {
		if ch == '{' {
			let mut name = String::new();
			for inner in chars.by_ref() {
				if inner == '}' || inner == ':' {
					break;
				}
				name.push(inner);
			}
			if !name.is_empty() {
				params.push(name);
			}
			// Skip rest until '}' if we broke on ':'
			if chars.peek().is_some() {
				for inner in chars.by_ref() {
					if inner == '}' {
						break;
					}
				}
			}
		}
	}
	params
}

/// A single route definition within `client_routes!`.
struct ClientRouteEntry {
	name: LitStr,
	_colon: Token![:],
	pattern: LitStr,
	_arrow: Token![=>],
	handler: Expr,
}

impl Parse for ClientRouteEntry {
	fn parse(input: ParseStream) -> Result<Self> {
		Ok(Self {
			name: input.parse()?,
			_colon: input.parse()?,
			pattern: input.parse()?,
			_arrow: input.parse()?,
			handler: input.parse()?,
		})
	}
}

/// Top-level input to `client_routes!`.
struct ClientRoutesInput {
	router_expr: Expr,
	_comma1: Token![,],
	app_label: Ident,
	_comma2: Token![,],
	routes: syn::punctuated::Punctuated<ClientRouteEntry, Token![,]>,
}

impl Parse for ClientRoutesInput {
	fn parse(input: ParseStream) -> Result<Self> {
		Ok(Self {
			router_expr: input.parse()?,
			_comma1: input.parse()?,
			app_label: input.parse()?,
			_comma2: input.parse()?,
			routes: syn::punctuated::Punctuated::parse_terminated(input)?,
		})
	}
}

pub(crate) fn client_routes_impl(input: TokenStream) -> Result<TokenStream> {
	let parsed: ClientRoutesInput = syn::parse2(input)?;

	let router_expr = &parsed.router_expr;
	let app_label = &parsed.app_label;
	let app_str = app_label.to_string();

	let mut route_registrations = Vec::new();
	let mut meta_macro_defs = Vec::new();
	let mut meta_idents = Vec::new();

	for entry in &parsed.routes {
		let route_name_str = entry.name.value();
		let pattern_str = entry.pattern.value();
		let handler = &entry.handler;

		// Validate route name is a valid Rust identifier
		if syn::parse_str::<syn::Ident>(&route_name_str).is_err() {
			return Err(syn::Error::new(
				entry.name.span(),
				format!(
					"Client route name `{route_name_str}` is not a valid Rust identifier. \
					 Route names must be valid identifiers (no hyphens, dots, or leading digits)."
				),
			));
		}

		let full_name = format!("{app_str}:{route_name_str}");
		let pattern_lit = &entry.pattern;
		let method_ident = Ident::new(&route_name_str, entry.name.span());

		// Generate runtime registration
		route_registrations.push(quote! {
			.named_route(#full_name, #pattern_lit, #handler)
		});

		// Generate metadata macro (same pattern as server-side __url_resolver_meta_xxx)
		let meta_macro_ident = Ident::new(
			&format!("__client_url_resolver_meta_{app_str}_{route_name_str}"),
			Span::call_site(),
		);

		let params = extract_url_params(&pattern_str);
		let param_strs: Vec<&str> = params.iter().map(|s| s.as_str()).collect();

		let meta_def = if params.is_empty() {
			quote! {
				#[doc(hidden)]
				macro_rules! #meta_macro_ident {
					($callback:ident, $app:ident) => {
						$callback!($app, #method_ident, #route_name_str, );
					};
				}
				pub(crate) use #meta_macro_ident;
			}
		} else {
			quote! {
				#[doc(hidden)]
				macro_rules! #meta_macro_ident {
					($callback:ident, $app:ident) => {
						$callback!($app, #method_ident, #route_name_str, #(#param_strs),* );
					};
				}
				pub(crate) use #meta_macro_ident;
			}
		};

		meta_macro_defs.push(meta_def);
		meta_idents.push(meta_macro_ident);
	}

	// Generate __for_each_client_url_resolver aggregator macro
	let aggregator_ident = Ident::new(
		&format!("__for_each_client_url_resolver"),
		Span::call_site(),
	);

	let aggregator = quote! {
		/// Invoke a callback macro for each client URL resolver in this app.
		/// Used by the `#[routes]` macro to build per-app client resolver structs.
		/// `$base` must be the absolute path to this module so that the metadata
		/// macros resolve at the call site.
		#[doc(hidden)]
		macro_rules! #aggregator_ident {
			($callback:ident, $app:ident, $base:path) => {
				#(
					$base :: #meta_idents ! ($callback, $app);
				)*
			};
		}
		pub(crate) use #aggregator_ident;
	};

	// The output:
	// 1. WASM: runtime route registrations
	// 2. Native-only: metadata macros in a `client_url_resolvers` module
	//    (parallel to server-side `url_resolvers` module)
	Ok(quote! {{
		// Metadata macros are native-only (consumed by #[routes] for per-app structs)
		#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
		pub mod client_url_resolvers {
			#(#meta_macro_defs)*
			#aggregator
		}

		// Runtime: register named routes on the ClientRouter (both native and WASM)
		#router_expr
			#(#route_registrations)*
	}})
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_extract_url_params_empty() {
		assert_eq!(extract_url_params("/login/"), Vec::<String>::new());
	}

	#[test]
	fn test_extract_url_params_single() {
		assert_eq!(extract_url_params("/users/{id}/"), vec!["id".to_string()]);
	}

	#[test]
	fn test_extract_url_params_multiple() {
		assert_eq!(
			extract_url_params("/users/{user_id}/posts/{post_id}/"),
			vec!["user_id".to_string(), "post_id".to_string()]
		);
	}

	#[test]
	fn test_extract_url_params_with_type_constraint() {
		assert_eq!(
			extract_url_params("/users/{id:int}/"),
			vec!["id".to_string()]
		);
	}
}
