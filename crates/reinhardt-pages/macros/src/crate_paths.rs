//! Helper functions for dynamic crate path resolution using proc_macro_crate
//!
//! These functions resolve crate paths at compile time, supporting various
//! dependency configurations (direct, via facade crate, etc.).

use proc_macro2::TokenStream;
use quote::quote;

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

	// Check what crates are available as dependencies
	let has_reinhardt_pages = matches!(crate_name("reinhardt-pages"), Ok(FoundCrate::Name(_)));
	let has_reinhardt = matches!(crate_name("reinhardt"), Ok(FoundCrate::Name(_)));
	let has_reinhardt_web = matches!(crate_name("reinhardt-web"), Ok(FoundCrate::Name(_)));

	// If both reinhardt-pages and reinhardt are available, use conditional compilation
	// This handles the case where the project has both as dependencies for dual-target builds
	if has_reinhardt_pages && (has_reinhardt || has_reinhardt_web) {
		return CratePathInfo {
			needs_conditional: true,
			use_statement: quote! {
				#[cfg(all(target_family = "wasm", target_os = "unknown"))]
				use ::reinhardt_pages as __reinhardt_pages;
				#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
				use ::reinhardt::pages as __reinhardt_pages;
			},
			ident: quote!(__reinhardt_pages),
		};
	}

	// Only reinhardt-pages is available
	if has_reinhardt_pages {
		return CratePathInfo {
			needs_conditional: false,
			use_statement: quote!(),
			ident: quote!(::reinhardt_pages),
		};
	}

	// Only reinhardt is available (via facade crate)
	if has_reinhardt {
		return CratePathInfo {
			needs_conditional: false,
			use_statement: quote!(),
			ident: quote!(::reinhardt::pages),
		};
	}

	// Only reinhardt-web is available (published package name)
	if has_reinhardt_web {
		return CratePathInfo {
			needs_conditional: false,
			use_statement: quote!(),
			ident: quote!(::reinhardt::pages),
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

/// Resolves the path to the reinhardt_di crate dynamically.
///
/// Uses the same strategy order as [`get_reinhardt_pages_crate`] to avoid
/// conditional dependency resolution issues.
pub(crate) fn get_reinhardt_di_crate() -> TokenStream {
	use proc_macro_crate::{FoundCrate, crate_name};

	// Try via reinhardt crate first (prioritized to avoid conditional dependency issues)
	match crate_name("reinhardt") {
		Ok(FoundCrate::Itself) => return quote!(::reinhardt::reinhardt_di),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident::reinhardt_di);
		}
		Err(_) => {}
	}

	// Try via reinhardt-web (published package name)
	match crate_name("reinhardt-web") {
		Ok(FoundCrate::Itself) => return quote!(::reinhardt::reinhardt_di),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident::reinhardt_di);
		}
		Err(_) => {}
	}

	// Try direct crate (for internal usage within reinhardt-di crate)
	match crate_name("reinhardt-di") {
		Ok(FoundCrate::Itself) => return quote!(::reinhardt_di),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident);
		}
		Err(_) => {}
	}

	// Final fallback - use reinhardt facade crate (re-exported module)
	quote!(::reinhardt::reinhardt_di)
}

/// Resolves the path to the reinhardt_http crate dynamically.
///
/// Uses the same strategy order as [`get_reinhardt_pages_crate`] to avoid
/// conditional dependency resolution issues.
pub(crate) fn get_reinhardt_http_crate() -> TokenStream {
	use proc_macro_crate::{FoundCrate, crate_name};

	// Try via reinhardt crate first (prioritized to avoid conditional dependency issues)
	match crate_name("reinhardt") {
		Ok(FoundCrate::Itself) => return quote!(::reinhardt::reinhardt_http),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident::reinhardt_http);
		}
		Err(_) => {}
	}

	// Try via reinhardt-web (published package name)
	match crate_name("reinhardt-web") {
		Ok(FoundCrate::Itself) => return quote!(::reinhardt::reinhardt_http),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident::reinhardt_http);
		}
		Err(_) => {}
	}

	// Try direct crate (for internal usage within reinhardt-http crate)
	match crate_name("reinhardt-http") {
		Ok(FoundCrate::Itself) => return quote!(::reinhardt_http),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident);
		}
		Err(_) => {}
	}

	// Final fallback - use reinhardt facade crate (re-exported module)
	quote!(::reinhardt::reinhardt_http)
}

/// Resolves the path to the reinhardt_core crate dynamically.
pub(crate) fn get_reinhardt_core_crate() -> TokenStream {
	use proc_macro_crate::{FoundCrate, crate_name};

	// Try via reinhardt crate first
	match crate_name("reinhardt") {
		Ok(FoundCrate::Itself) => return quote!(::reinhardt::reinhardt_core),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident::reinhardt_core);
		}
		Err(_) => {}
	}

	// Try via reinhardt-web (published package name)
	match crate_name("reinhardt-web") {
		Ok(FoundCrate::Itself) => return quote!(::reinhardt::reinhardt_core),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident::reinhardt_core);
		}
		Err(_) => {}
	}

	// Try direct crate
	match crate_name("reinhardt-core") {
		Ok(FoundCrate::Itself) => return quote!(::reinhardt_core),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident);
		}
		Err(_) => {}
	}

	// Final fallback
	quote!(::reinhardt_core)
}
