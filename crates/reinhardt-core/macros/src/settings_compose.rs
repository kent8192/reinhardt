//! Handler for `#[settings(key: Type | Type | key: Type)]`

use crate::settings_parser::{FragmentEntry, parse_settings_attr};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use std::collections::HashSet;
use syn::{ItemStruct, Result};

/// Convert CamelCase to snake_case.
///
/// Walk characters left to right. Insert `_` before an uppercase letter
/// when the previous character is lowercase, or when it begins a new word
/// after a run of uppercase letters.
///
/// Examples: `"Core"` → `"core"`, `"StaticFiles"` → `"static_files"`,
/// `"I18n"` → `"i18n"`, `"HTTPSProxy"` → `"https_proxy"`.
fn camel_to_snake(s: &str) -> String {
	let mut result = String::with_capacity(s.len() + 4);
	let chars: Vec<char> = s.chars().collect();

	for (i, &ch) in chars.iter().enumerate() {
		if ch.is_uppercase() {
			if i > 0 {
				let prev = chars[i - 1];
				if prev.is_lowercase() || prev.is_ascii_digit() {
					// aB → a_b
					result.push('_');
				} else if prev.is_uppercase()
					&& chars.get(i + 1).is_some_and(|next| next.is_lowercase())
				{
					// ABc → a_bc (acronym boundary)
					result.push('_');
				}
			}
			result.push(ch.to_lowercase().next().unwrap());
		} else {
			result.push(ch);
		}
	}

	result
}

/// Rust keywords that cannot be used as field names.
///
/// Includes strict keywords, reserved keywords, and weak keywords.
/// Mirrors the keyword set in `crates/reinhardt-db/src/migrations/introspect/naming.rs`.
const RUST_KEYWORDS: &[&str] = &[
	// Strict keywords
	"as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern",
	"false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub",
	"ref", "return", "self", "Self", "static", "struct", "super", "trait", "true", "type",
	"unsafe", "use", "where", "while",
	// Reserved keywords (may be used in future)
	"abstract", "become", "box", "do", "final", "macro", "override", "priv", "try", "typeof",
	"unsized", "virtual", "yield", // Weak keywords (context-sensitive)
	"union",
];

/// Strip `Settings` suffix and convert CamelCase prefix to snake_case.
///
/// Returns error if:
/// - Type does not end with `Settings`
/// - Prefix is empty (type is exactly `Settings`)
/// - Inferred name is a Rust keyword
fn infer_field_name(type_name: &str) -> std::result::Result<String, String> {
	let prefix = type_name.strip_suffix("Settings").ok_or_else(|| {
		format!(
			"Type `{}` does not end with `Settings`. Use explicit syntax: `field_name: {}`",
			type_name, type_name
		)
	})?;

	if prefix.is_empty() {
		return Err(
			"Type `Settings` has an empty prefix after stripping `Settings` suffix.".to_string(),
		);
	}

	let field_name = camel_to_snake(prefix);

	if RUST_KEYWORDS.contains(&field_name.as_str()) {
		return Err(format!(
			"Type `{}` infers field name `{}`, which is a Rust keyword. Use explicit syntax: `{}_field: {}`",
			type_name, field_name, field_name, type_name
		));
	}

	Ok(field_name)
}

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
			FragmentEntry::TypeOnly(type_name) => {
				let key = infer_field_name(type_name)
					.map_err(|msg| syn::Error::new(proc_macro2::Span::call_site(), msg))?;
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
				includes.push((key, type_name.clone()));
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

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	#[case("Core", "core")]
	#[case("Cache", "cache")]
	#[case("StaticFiles", "static_files")]
	#[case("I18n", "i18n")]
	#[case("Cors", "cors")]
	#[case("X", "x")]
	#[case("HTTPSProxy", "https_proxy")]
	fn test_camel_to_snake(#[case] input: &str, #[case] expected: &str) {
		// Act
		let result = camel_to_snake(input);

		// Assert
		assert_eq!(result, expected);
	}

	#[rstest]
	#[case("CoreSettings", Ok("core"))]
	#[case("CacheSettings", Ok("cache"))]
	#[case("StaticFilesSettings", Ok("static_files"))]
	#[case("I18nSettings", Ok("i18n"))]
	#[case("CorsSettings", Ok("cors"))]
	#[case("XSettings", Ok("x"))]
	fn test_infer_field_name_success(
		#[case] input: &str,
		#[case] expected: std::result::Result<&str, &str>,
	) {
		// Act
		let result = infer_field_name(input);

		// Assert
		assert_eq!(result, expected.map(String::from).map_err(String::from));
	}

	#[rstest]
	#[case("MyCustomConfig", "does not end with `Settings`")]
	#[case("Settings", "empty prefix")]
	#[case("StaticSettings", "Rust keyword")]
	fn test_infer_field_name_error(#[case] input: &str, #[case] expected_contains: &str) {
		// Act
		let result = infer_field_name(input);

		// Assert
		assert!(result.is_err());
		assert!(
			result.as_ref().unwrap_err().contains(expected_contains),
			"Error message {:?} should contain {:?}",
			result.unwrap_err(),
			expected_contains
		);
	}
}
