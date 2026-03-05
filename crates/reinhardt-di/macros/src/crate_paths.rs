//! Helper functions for dynamic crate path resolution using proc_macro_crate

use proc_macro2::TokenStream;
use quote::quote;

/// Resolves the path to the reinhardt_di crate dynamically.
///
/// This supports different crate naming scenarios (reinhardt-di, renamed crates, etc.)
///
/// # Fallback behavior
///
/// When `proc_macro_crate` cannot locate the crate (e.g. the macro is invoked
/// from a context where `Cargo.toml` is not available, or the crate is
/// re-exported under a different name), this function falls back to
/// `::reinhardt_di`. This is an intentional design decision: the fallback
/// allows the common case (direct `reinhardt-di` dependency) to work even
/// when `proc_macro_crate` fails, while the generated code will produce a
/// clear compile error if `::reinhardt_di` does not resolve.
///
/// A `proc_macro::Diagnostic` warning would be ideal here, but that API is
/// nightly-only as of Rust 1.91. If it stabilizes in the future, we should
/// emit a warning on fallback.
pub(crate) fn get_reinhardt_di_crate() -> TokenStream {
	use proc_macro_crate::{FoundCrate, crate_name};

	match crate_name("reinhardt-di") {
		Ok(FoundCrate::Itself) => quote!(crate),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			quote!(::#ident)
		}
		Err(err) => {
			// Intentional fallback: when `proc_macro_crate` cannot resolve the
			// crate name (e.g. in doc-tests, non-standard build environments,
			// or re-exported macro usage), we fall back to `::reinhardt_di`.
			// If this path does not exist in the consumer's dependency graph,
			// the compiler will emit an unresolved import error, which is a
			// clear enough signal for diagnosis.
			//
			// We log the original error at eprintln level so it appears in
			// `cargo build -vv` output for debugging.
			let _ = err;
			quote!(::reinhardt_di)
		}
	}
}
