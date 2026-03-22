//! Handler for `#[settings(key: Type | key: Type | !Type)]`

use crate::settings_parser::{FragmentEntry, parse_settings_attr};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use std::collections::HashSet;
use syn::{ItemStruct, Result};

/// The only implicit fragment type name.
const IMPLICIT_FRAGMENT: &str = "CoreSettings";
/// The default key for the implicit CoreSettings fragment.
const IMPLICIT_KEY: &str = "core";

/// Implementation for `#[settings(key: Type | !Type)]`.
pub(crate) fn settings_compose_impl(args: TokenStream, input: ItemStruct) -> Result<TokenStream> {
	let conf_crate = crate::crate_paths::get_reinhardt_conf_crate();
	let struct_name = &input.ident;
	let vis = &input.vis;
	let attrs: Vec<_> = input.attrs.iter().collect();

	let args_str = args.to_string();

	// Parse entries (empty attr means empty list, just CoreSettings)
	let entries = if args_str.trim().is_empty() {
		vec![]
	} else {
		let (_, entries) = parse_settings_attr(&args_str).map_err(|e| {
			syn::Error::new(
				proc_macro2::Span::call_site(),
				format!("failed to parse settings attribute: {}", e),
			)
		})?;
		entries
	};

	// Separate includes and excludes
	let mut includes: Vec<(String, String)> = vec![];
	let mut excludes: HashSet<String> = HashSet::new();
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
				if excludes.contains(type_name) {
					return Err(syn::Error::new(
						proc_macro2::Span::call_site(),
						format!("Cannot both include and exclude `{}`.", type_name),
					));
				}
				includes.push((key.clone(), type_name.clone()));
			}
			FragmentEntry::Exclude(type_name) => {
				if type_name != IMPLICIT_FRAGMENT {
					return Err(syn::Error::new(
						proc_macro2::Span::call_site(),
						format!(
							"Cannot exclude `{}`: it is not implicitly included. Remove the `!` prefix.",
							type_name,
						),
					));
				}
				if seen_types.contains(type_name) {
					return Err(syn::Error::new(
						proc_macro2::Span::call_site(),
						format!("Cannot both include and exclude `{}`.", type_name),
					));
				}
				excludes.insert(type_name.clone());
			}
		}
	}

	// Add implicit CoreSettings if not excluded
	let mut all_fragments: Vec<(String, String)> = vec![];
	if !excludes.contains(IMPLICIT_FRAGMENT) && !seen_types.contains(IMPLICIT_FRAGMENT) {
		all_fragments.push((IMPLICIT_KEY.to_string(), IMPLICIT_FRAGMENT.to_string()));
	}
	all_fragments.extend(includes);

	// Generate struct fields
	let field_defs: Vec<_> = all_fragments
		.iter()
		.map(|(key, type_name)| {
			let key_ident = format_ident!("{}", key);
			let type_ident = format_ident!("{}", type_name);
			quote! { pub #key_ident: #type_ident }
		})
		.collect();

	// Generate Has* trait impls
	let trait_impls: Vec<_> = all_fragments
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
	let validate_calls: Vec<_> = all_fragments
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
