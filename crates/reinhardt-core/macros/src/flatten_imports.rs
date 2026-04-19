//! `flatten_imports!` function-like proc macro for multi-file view modules.
//!
//! When used in a view module that uses per-file endpoint organization,
//! this macro generates `pub use submod::*;` for each `pub mod` declaration.
//! This re-exports endpoint functions and their `__url_resolver_*` modules
//! so that `#[url_patterns]` can discover resolvers using the standard
//! parent-module path.
//!
//! # Example
//!
//! ```rust,ignore
//! // views.rs
//! use reinhardt::flatten_imports;
//!
//! flatten_imports! {
//!     pub mod login;
//!     pub mod register;
//! }
//!
//! // Generates:
//! //   pub mod login;
//! //   pub mod register;
//! //   pub use login::*;
//! //   pub use register::*;
//! ```

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Item, parse2};

/// Implementation for the `flatten_imports!` proc macro.
///
/// Scans the macro input for `pub mod name;` declarations and appends
/// `pub use name::*;` for each one. This brings endpoint functions and their
/// generated `__url_resolver_*` modules into the parent module scope, enabling
/// `#[url_patterns]` to resolve them with the standard path convention.
pub(crate) fn flatten_imports_impl(input: TokenStream) -> syn::Result<TokenStream> {
	let items: Vec<Item> = {
		let file: syn::File = parse2(input.clone())
			.map_err(|e| syn::Error::new(e.span(), format!("flatten_imports: {e}")))?;
		file.items
	};

	// Collect pub mod declarations (external modules only, i.e. `pub mod name;`)
	let mut reexports = Vec::new();
	for item in &items {
		if let Item::Mod(item_mod) = item {
			// Only process `pub mod name;` (no body = external module file)
			let is_pub = matches!(item_mod.vis, syn::Visibility::Public(_));
			let is_external = item_mod.content.is_none();
			if is_pub && is_external {
				let mod_name = &item_mod.ident;
				reexports.push(quote! {
					pub use #mod_name::*;
				});
			}
		}
	}

	// Emit original items + reexports
	Ok(quote! {
		#input
		#(#reexports)*
	})
}
