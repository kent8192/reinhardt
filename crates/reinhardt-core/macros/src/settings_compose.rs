//! Handler for `#[settings(key: Type | key: Type)]`

use crate::settings_parser::{FragmentEntry, parse_settings_attr};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use std::collections::HashSet;
use syn::{ItemStruct, Result};

/// Implementation for `#[settings(key: Type)]`.
pub(crate) fn settings_compose_impl(args: TokenStream, input: ItemStruct) -> Result<TokenStream> {
	let conf_crate = crate::crate_paths::get_reinhardt_conf_crate();
	let struct_name = &input.ident;
	let vis = &input.vis;
	let attrs: Vec<_> = input.attrs.iter().collect();

	let args_str = args.to_string();

	// Empty attribute is an error — at least one fragment must be specified
	if args_str.trim().is_empty() {
		return Err(syn::Error::new(
			proc_macro2::Span::call_site(),
			"#[settings()] requires at least one fragment. Use `#[settings(core: CoreSettings)]` for core-only settings.",
		));
	}

	let (_, entries) = parse_settings_attr(&args_str).map_err(|e| {
		syn::Error::new(
			proc_macro2::Span::call_site(),
			format!("failed to parse settings attribute: {}", e),
		)
	})?;

	// Collect includes; exclusion syntax is no longer supported
	let mut includes: Vec<(String, String)> = vec![];
	let mut seen_keys: HashSet<String> = HashSet::new();
	let mut seen_types: HashSet<String> = HashSet::new();

	for entry in &entries {
		match entry {
			FragmentEntry::Include { key, type_name } => {
				if !seen_keys.insert(key.clone()) {
					return Err(syn::Error::new(
						proc_macro2::Span::call_site(),
						format!("Duplicate field name `{}`.", key),
					));
				}
				if !seen_types.insert(type_name.clone()) {
					return Err(syn::Error::new(
						proc_macro2::Span::call_site(),
						format!("Duplicate fragment type `{}`.", type_name),
					));
				}
				includes.push((key.clone(), type_name.clone()));
			}
			FragmentEntry::Exclude(type_name) => {
				return Err(syn::Error::new(
					proc_macro2::Span::call_site(),
					format!(
						"Exclusion syntax `!{}` is no longer supported. Simply omit the fragment instead.",
						type_name,
					),
				));
			}
		}
	}

	// Generate struct fields
	let field_defs: Vec<_> = includes
		.iter()
		.map(|(key, type_name)| {
			let key_ident = format_ident!("{}", key);
			let type_ident = format_ident!("{}", type_name);
			quote! { pub #key_ident: #type_ident }
		})
		.collect();

	// Generate Has* trait impls
	let trait_impls: Vec<_> = includes
		.iter()
		.map(|(key, type_name)| {
			let key_ident = format_ident!("{}", key);
			let type_ident = format_ident!("{}", type_name);
			let trait_name = format_ident!("Has{}", type_name);
			quote! {
				impl #trait_name for #struct_name {
					fn #key_ident(&self) -> &#type_ident {
						&self.#key_ident
					}
				}
			}
		})
		.collect();

	// Generate validate() method calls
	let validate_calls: Vec<_> = includes
		.iter()
		.map(|(key, _)| {
			let key_ident = format_ident!("{}", key);
			quote! {
				self.#key_ident.validate(profile)?;
			}
		})
		.collect();

	Ok(quote! {
		#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
		#(#attrs)*
		#vis struct #struct_name {
			#(#field_defs,)*
		}

		#(#trait_impls)*

		impl #struct_name {
			/// Validate all fragments against the given profile.
			pub fn validate(
				&self,
				profile: &#conf_crate::settings::profile::Profile,
			) -> #conf_crate::settings::validation::ValidationResult {
				#(#validate_calls)*
				Ok(())
			}
		}
	})
}
