//! Helper functions for dynamic crate path resolution using proc_macro_crate

use proc_macro2::TokenStream;
use quote::quote;

fn crate_path(package_name: &str) -> Option<TokenStream> {
	use proc_macro_crate::{FoundCrate, crate_name};

	match crate_name(package_name) {
		Ok(FoundCrate::Itself) => Some(quote!(crate)),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			Some(quote!(::#ident))
		}
		Err(_) => None,
	}
}

fn facade_module_path(module: &str) -> Option<TokenStream> {
	use proc_macro_crate::{FoundCrate, crate_name};

	let module_ident = syn::Ident::new(module, proc_macro2::Span::call_site());

	match crate_name("reinhardt-web") {
		Ok(FoundCrate::Itself) => Some(quote!(crate::#module_ident)),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			Some(quote!(::#ident::#module_ident))
		}
		Err(_) => None,
	}
}

/// Resolves the path to the reinhardt_di crate dynamically.
///
/// This supports different crate naming scenarios (reinhardt-di, renamed crates, etc.)
pub(crate) fn get_reinhardt_di_crate() -> TokenStream {
	crate_path("reinhardt-di")
		.or_else(|| facade_module_path("di"))
		.unwrap_or_else(|| quote!(::reinhardt_di))
}

/// Resolves the path to the reinhardt_grpc crate dynamically.
///
/// This supports different crate naming scenarios (reinhardt-grpc, renamed crates, etc.)
pub(crate) fn get_reinhardt_grpc_crate() -> TokenStream {
	crate_path("reinhardt-grpc")
		.or_else(|| facade_module_path("grpc"))
		.unwrap_or_else(|| quote!(::reinhardt_grpc))
}
