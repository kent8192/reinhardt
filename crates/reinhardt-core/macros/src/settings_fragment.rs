//! Handler for `#[settings(fragment = true, section = "...")]`

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{ItemStruct, LitStr, Result};

/// Implementation for `#[settings(fragment = true, section = "...")]`.
pub(crate) fn settings_fragment_impl(args: TokenStream, input: ItemStruct) -> Result<TokenStream> {
	let conf_crate = crate::crate_paths::get_reinhardt_conf_crate();

	// Parse section from args
	let mut section: Option<String> = None;

	let parser = syn::meta::parser(|meta| {
		if meta.path.is_ident("fragment") {
			let _: syn::LitBool = meta.value()?.parse()?;
			Ok(())
		} else if meta.path.is_ident("section") {
			let lit: LitStr = meta.value()?.parse()?;
			section = Some(lit.value());
			Ok(())
		} else {
			Err(meta.error("expected `fragment = true` or `section = \"...\"`"))
		}
	});

	syn::parse::Parser::parse2(parser, args)?;

	let section = section.ok_or_else(|| {
		syn::Error::new(
			proc_macro2::Span::call_site(),
			"`section = \"...\"` is required for `#[settings(fragment = true)]`",
		)
	})?;

	let struct_name = &input.ident;
	let vis = &input.vis;
	let trait_name = format_ident!("Has{}", struct_name);
	let method_name = format_ident!("{}", section);

	// Check if derives are already present
	let has_derive = input.attrs.iter().any(|a| a.path().is_ident("derive"));

	let derive_attr = if has_derive {
		quote! {}
	} else {
		quote! { #[derive(Clone, Debug, serde::Serialize, serde::Deserialize)] }
	};

	// Preserve existing attributes
	let attrs = &input.attrs;
	let fields = &input.fields;
	let semi_token = &input.semi_token;

	// Handle both named and unit structs
	let struct_body = if semi_token.is_some() {
		quote! { ; }
	} else {
		quote! { #fields }
	};

	Ok(quote! {
		#derive_attr
		#(#attrs)*
		#vis struct #struct_name #struct_body

		impl #conf_crate::settings::fragment::SettingsFragment for #struct_name {
			fn section() -> &'static str {
				#section
			}
		}

		/// Trait for accessing the settings fragment from a composed settings type.
		#vis trait #trait_name {
			/// Get a reference to the settings fragment.
			fn #method_name(&self) -> &#struct_name;
		}
	})
}
