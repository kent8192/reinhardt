//! Handler for `#[settings(key: Type | key: Type)]`

use crate::settings_parser::{FragmentEntry, parse_settings_attr};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use std::collections::HashSet;
use syn::{ItemStruct, Result};

/// Built-in fragment types that are re-exported from `reinhardt_conf`.
///
/// These types are resolved via the `reinhardt_conf` crate path in generated
/// code so that users do not need to manually import them.  User-defined
/// fragment types (not in this list) are emitted as bare identifiers and must
/// be imported by the caller.
const BUILTIN_FRAGMENTS: &[&str] = &[
	"CoreSettings",
	"CacheSettings",
	"ContactSettings",
	"CorsSettings",
	"EmailSettings",
	"I18nSettings",
	"LoggingSettings",
	"MediaSettings",
	"SecuritySettings",
	"SessionSettings",
	"StaticSettings",
	"TemplateSettings",
];

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
	//
	// Each fragment field is `#[serde(flatten)]`-ed so that flat settings
	// sources (TOML files, DefaultSource key-value pairs) deserialize
	// directly into the fragment without requiring a nested section key.
	// This mirrors the behavior of the deprecated `Settings` struct.
	let field_defs: Vec<_> = includes
		.iter()
		.map(|(key, type_name)| {
			let key_ident = format_ident!("{}", key);
			let type_path = resolve_fragment_type(type_name, &conf_crate);
			quote! {
				#[serde(flatten)]
				pub #key_ident: #type_path
			}
		})
		.collect();

	// Generate Has* trait impls
	let trait_impls: Vec<_> = includes
		.iter()
		.map(|(key, type_name)| {
			let key_ident = format_ident!("{}", key);
			let type_path = resolve_fragment_type(type_name, &conf_crate);
			let trait_path = resolve_fragment_trait(type_name, &conf_crate);
			quote! {
				impl #trait_path for #struct_name {
					fn #key_ident(&self) -> &#type_path {
						&self.#key_ident
					}
				}
			}
		})
		.collect();

	// Generate validate() method calls using fully-qualified path
	// to avoid requiring SettingsFragment import at the call site
	let validate_calls: Vec<_> = includes
		.iter()
		.map(|(key, _)| {
			let key_ident = format_ident!("{}", key);
			quote! {
				#conf_crate::settings::fragment::SettingsFragment::validate(&self.#key_ident, profile)?;
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

/// Returns a token stream for a fragment type path.
///
/// Built-in fragment types (defined in [`BUILTIN_FRAGMENTS`]) are emitted as
/// fully qualified paths through `conf_crate` (e.g. `reinhardt_conf::CoreSettings`).
/// User-defined types are emitted as bare identifiers.
fn resolve_fragment_type(type_name: &str, conf_crate: &TokenStream) -> TokenStream {
	let type_ident = format_ident!("{}", type_name);
	if BUILTIN_FRAGMENTS.contains(&type_name) {
		quote! { #conf_crate::#type_ident }
	} else {
		quote! { #type_ident }
	}
}

/// Returns a token stream for a `Has{Fragment}` trait path.
///
/// Built-in fragment traits are emitted as fully qualified paths through
/// `conf_crate`.  User-defined traits are emitted as bare identifiers.
fn resolve_fragment_trait(type_name: &str, conf_crate: &TokenStream) -> TokenStream {
	let trait_ident = format_ident!("Has{}", type_name);
	if BUILTIN_FRAGMENTS.contains(&type_name) {
		quote! { #conf_crate::#trait_ident }
	} else {
		quote! { #trait_ident }
	}
}
