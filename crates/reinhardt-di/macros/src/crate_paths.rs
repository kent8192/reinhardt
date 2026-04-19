//! Helper functions for dynamic crate path resolution using proc_macro_crate

use proc_macro2::TokenStream;
use quote::quote;

/// Resolves the path to the reinhardt_di crate dynamically.
///
/// This supports different crate naming scenarios (reinhardt-di, renamed crates,
/// or access via the `reinhardt` facade crate).
///
/// # Resolution order
///
/// 1. Direct `reinhardt-di` dependency (exact match or renamed)
/// 2. Via the `reinhardt` facade crate (`reinhardt::reinhardt_di`)
/// 3. Via the `reinhardt-web` published package name (`reinhardt_web::reinhardt_di`)
/// 4. Final fallback to `::reinhardt_di`
///
/// The multi-step fallback ensures that consumers using the `reinhardt` facade
/// crate (instead of depending on `reinhardt-di` directly) can still use
/// DI macros without manual path configuration.
///
/// If none of the above resolve, the final `::reinhardt_di` fallback will
/// produce a clear compile error for diagnosis.
pub(crate) fn get_reinhardt_di_crate() -> TokenStream {
	use proc_macro_crate::{FoundCrate, crate_name};

	// Try direct crate first
	match crate_name("reinhardt-di") {
		Ok(FoundCrate::Itself) => return quote!(crate),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident);
		}
		Err(_) => {}
	}

	// Try via reinhardt facade crate
	match crate_name("reinhardt") {
		Ok(FoundCrate::Itself) => return quote!(crate::reinhardt_di),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident::reinhardt_di);
		}
		Err(_) => {}
	}

	// Try via reinhardt-web (published package name)
	match crate_name("reinhardt-web") {
		Ok(FoundCrate::Itself) => return quote!(crate::reinhardt_di),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident::reinhardt_di);
		}
		Err(_) => {}
	}

	// Final fallback
	quote!(::reinhardt_di)
}
