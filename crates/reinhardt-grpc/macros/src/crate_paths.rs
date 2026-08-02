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
		// The facade package exposes the `reinhardt` library target only when
		// its dependency declaration has no explicit package alias.
		Ok(FoundCrate::Name(name))
			if name == "reinhardt_web" && has_package_only_facade_dependency() =>
		{
			Some(quote!(::reinhardt::#module_ident))
		}
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			Some(quote!(::#ident::#module_ident))
		}
		Err(_) => None,
	}
}

fn has_package_only_facade_dependency() -> bool {
	use toml_edit::{DocumentMut, Item};

	let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") else {
		return false;
	};
	let manifest_path = std::path::Path::new(&manifest_dir).join("Cargo.toml");
	let Ok(manifest) = std::fs::read_to_string(manifest_path) else {
		return false;
	};
	let Ok(document) = manifest.parse::<DocumentMut>() else {
		return false;
	};

	["dependencies", "dev-dependencies"]
		.into_iter()
		.any(|table_name| {
			document
				.get(table_name)
				.and_then(Item::as_table_like)
				.is_some_and(has_unaliased_facade_dependency)
		}) || document
		.get("target")
		.and_then(Item::as_table_like)
		.is_some_and(|targets| {
			targets.iter().any(|(_, target)| {
				target.as_table_like().is_some_and(|target_table| {
					["dependencies", "dev-dependencies"]
						.into_iter()
						.any(|table_name| {
							target_table
								.get(table_name)
								.and_then(Item::as_table_like)
								.is_some_and(has_unaliased_facade_dependency)
						})
				})
			})
		})
}

fn has_unaliased_facade_dependency(dependencies: &dyn toml_edit::TableLike) -> bool {
	dependencies
		.get("reinhardt-web")
		.is_some_and(|dependency| dependency.get("package").is_none())
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
