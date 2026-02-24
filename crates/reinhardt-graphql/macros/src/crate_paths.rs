//! Helper functions for dynamic crate path resolution using proc_macro_crate

use proc_macro2::TokenStream;
use quote::quote;

/// Resolves the path to the reinhardt_di crate dynamically.
///
/// This supports different crate naming scenarios (reinhardt-di, renamed crates, etc.)
/// Returns an error if the crate cannot be found in Cargo.toml.
pub(crate) fn get_reinhardt_di_crate() -> syn::Result<TokenStream> {
	use proc_macro_crate::{FoundCrate, crate_name};

	match crate_name("reinhardt-di") {
		Ok(FoundCrate::Itself) => Ok(quote!(::reinhardt_di)),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			Ok(quote!(::#ident))
		}
		Err(e) => Err(syn::Error::new(
			proc_macro2::Span::call_site(),
			format!(
				"failed to resolve `reinhardt-di` crate: {}. Ensure it is listed in Cargo.toml dependencies.",
				e
			),
		)),
	}
}

/// Resolves the path to the reinhardt_graphql crate dynamically.
///
/// This supports different crate naming scenarios (reinhardt-graphql, renamed crates, etc.)
/// Returns an error if the crate cannot be found in Cargo.toml.
pub(crate) fn get_reinhardt_graphql_crate() -> syn::Result<TokenStream> {
	use proc_macro_crate::{FoundCrate, crate_name};

	match crate_name("reinhardt-graphql") {
		Ok(FoundCrate::Itself) => Ok(quote!(::reinhardt_graphql)),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			Ok(quote!(::#ident))
		}
		Err(e) => Err(syn::Error::new(
			proc_macro2::Span::call_site(),
			format!(
				"failed to resolve `reinhardt-graphql` crate: {}. Ensure it is listed in Cargo.toml dependencies.",
				e
			),
		)),
	}
}
