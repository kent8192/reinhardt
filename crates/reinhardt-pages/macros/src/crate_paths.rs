//! Helper functions for dynamic crate path resolution using proc_macro_crate
//!
//! These functions resolve crate paths at compile time, supporting various
//! dependency configurations (direct, via facade crate, etc.).

use proc_macro2::TokenStream;
use quote::quote;

fn named_crate_path(name: String) -> TokenStream {
	let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
	quote!(::#ident)
}

/// Information about how to reference the reinhardt_pages crate.
pub(crate) struct CratePathInfo {
	/// Whether conditional compilation is needed (both reinhardt and reinhardt-pages are dependencies)
	pub needs_conditional: bool,
	/// The use statement(s) to emit (may include `#[cfg(...)]` attributes)
	pub use_statement: TokenStream,
	/// The identifier to use when referencing the crate (e.g., `__reinhardt_pages`)
	pub ident: TokenStream,
}

/// Resolves the path to the reinhardt_pages crate dynamically.
///
/// Since proc macros cannot detect the target architecture at runtime (they run on the host),
/// this function generates conditional code using `#[cfg(all(target_family = "wasm", target_os = "unknown"))]` that the
/// Rust compiler will select at compile time.
///
/// # Strategy
///
/// 1. Internal crate usage (`Itself`): Use `::reinhardt_pages` absolute path (doc test compatible)
/// 2. Both `reinhardt` and `reinhardt-pages` are dependencies: Generate conditional code
///    - WASM: `use ::reinhardt_pages`
///    - Server: `use ::reinhardt::pages`
/// 3. Only `reinhardt-pages`: Use it directly
/// 4. Only `reinhardt`: Use `::reinhardt::pages`
/// 5. Fallback: Use `::reinhardt_pages`
pub(crate) fn get_reinhardt_pages_crate_info() -> CratePathInfo {
	get_reinhardt_pages_crate_info_with_alias(&syn::Ident::new(
		"__reinhardt_pages",
		proc_macro2::Span::call_site(),
	))
}

pub(crate) fn get_reinhardt_pages_crate_info_with_alias(alias: &syn::Ident) -> CratePathInfo {
	use proc_macro_crate::{FoundCrate, crate_name};

	// Check for internal crate usage first.
	// Use absolute path `::reinhardt_pages` instead of `crate` for doc test compatibility.
	// In doc tests, `crate` refers to the test binary, not `reinhardt_pages`.
	// The target crate must have `extern crate self as reinhardt_pages;` for this to work.
	if let Ok(FoundCrate::Itself) = crate_name("reinhardt-pages") {
		return CratePathInfo {
			needs_conditional: false,
			use_statement: quote!(),
			ident: quote!(::reinhardt_pages),
		};
	}

	let direct_pages = match crate_name("reinhardt-pages") {
		Ok(FoundCrate::Itself) => Some(quote!(::reinhardt_pages)),
		Ok(FoundCrate::Name(name)) => Some(named_crate_path(name)),
		Err(_) => None,
	};
	let facade = match crate_name("reinhardt") {
		Ok(FoundCrate::Itself) => Some(quote!(::reinhardt)),
		Ok(FoundCrate::Name(name)) => Some(named_crate_path(name)),
		Err(_) => match crate_name("reinhardt-web") {
			Ok(FoundCrate::Itself) => Some(quote!(::reinhardt)),
			Ok(FoundCrate::Name(name)) => Some(named_crate_path(name)),
			Err(_) => None,
		},
	};

	// If both reinhardt-pages and reinhardt are available, use conditional compilation
	// This handles the case where the project has both as dependencies for dual-target builds
	if let (Some(direct_pages), Some(facade)) = (&direct_pages, &facade) {
		return CratePathInfo {
			needs_conditional: true,
			use_statement: quote! {
				#[cfg(all(target_family = "wasm", target_os = "unknown"))]
				use #direct_pages as #alias;
				#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
				use #facade::pages as #alias;
			},
			ident: quote!(#alias),
		};
	}

	// Only reinhardt-pages is available
	if let Some(direct_pages) = direct_pages {
		return CratePathInfo {
			needs_conditional: false,
			use_statement: quote!(),
			ident: direct_pages,
		};
	}

	// Only the facade is available.
	if let Some(facade) = facade {
		return CratePathInfo {
			needs_conditional: false,
			use_statement: quote!(),
			ident: quote!(#facade::pages),
		};
	}

	// Fallback - assume reinhardt_pages is available
	CratePathInfo {
		needs_conditional: false,
		use_statement: quote!(),
		ident: quote!(::reinhardt_pages),
	}
}

/// Legacy function for backwards compatibility.
/// Use `get_reinhardt_pages_crate_info()` for new code that needs conditional compilation.
pub(crate) fn get_reinhardt_pages_crate() -> TokenStream {
	let info = get_reinhardt_pages_crate_info();
	if info.needs_conditional {
		// For legacy callers that can't handle conditional compilation,
		// prefer the server path (most common case for non-page! macro usage)
		quote!(::reinhardt::pages)
	} else {
		info.ident
	}
}
