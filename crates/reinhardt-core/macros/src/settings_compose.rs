//! Handler for `#[settings(key: Type | Type | key: Type)]`

use crate::settings_parser::{FieldOverride, FragmentEntry, PolicyKind, parse_settings_attr};
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

	// Collect includes with overrides; exclusion syntax is no longer supported
	let mut includes: Vec<(String, String, Vec<FieldOverride>)> = vec![];
	let mut seen_keys: HashSet<String> = HashSet::new();
	let mut seen_types: HashSet<String> = HashSet::new();

	for entry in &entries {
		match entry {
			FragmentEntry::Include {
				key,
				type_name,
				overrides,
			} => {
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
				// Check for duplicate field names within the override block
				let mut seen_override_fields: HashSet<String> = HashSet::new();
				for ovr in overrides {
					if !seen_override_fields.insert(ovr.field_name.clone()) {
						return Err(syn::Error::new(
							proc_macro2::Span::call_site(),
							format!(
								"Duplicate override for field `{}` in fragment `{}`.",
								ovr.field_name, type_name,
							),
						));
					}
				}
				includes.push((key.clone(), type_name.clone(), overrides.clone()));
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
				includes.push((key, type_name.clone(), vec![]));
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
	// Each fragment field is deserialized from a TOML section matching
	// the fragment's `section()` name (e.g., `[core]` → `core: CoreSettings`).
	// This allows TOML files to use the conventional `[section]` structure.
	let field_defs: Vec<_> = includes
		.iter()
		.map(|(key, type_name, _)| {
			let key_ident = format_ident!("{}", key);
			let type_path = resolve_fragment_type(type_name, &conf_crate);
			quote! {
				pub #key_ident: #type_path
			}
		})
		.collect();

	// Generate HasSettings<F> impls for each fragment
	let trait_impls: Vec<_> = includes
		.iter()
		.map(|(key, type_name, _)| {
			let key_ident = format_ident!("{}", key);
			let type_path = resolve_fragment_type(type_name, &conf_crate);
			quote! {
				impl #conf_crate::settings::fragment::HasSettings<#type_path> for #struct_name {
					fn get_settings(&self) -> &#type_path {
						&self.#key_ident
					}
				}
			}
		})
		.collect();

	// Generate validate() method calls using fully-qualified SettingsFragment path.
	// For fragments with custom validation (validate = false), the macro-generated
	// SettingsFragment::validate delegates to SettingsValidation::validate automatically.
	let validate_calls: Vec<_> = includes
		.iter()
		.map(|(key, _, _)| {
			let key_ident = format_ident!("{}", key);
			quote! {
				#conf_crate::settings::fragment::SettingsFragment::validate(&self.#key_ident, profile)?;
			}
		})
		.collect();

	// Generate resolved_*_policies() methods for fragments with overrides,
	// and compile-time field existence assertions for override targets.
	let mut resolved_methods: Vec<TokenStream> = vec![];
	let mut field_assertions: Vec<TokenStream> = vec![];

	for (key, type_name, overrides) in &includes {
		if overrides.is_empty() {
			continue;
		}

		let type_path = resolve_fragment_type(type_name, &conf_crate);
		let method_name = format_ident!("resolved_{}_policies", key);

		// Generate match arms for each override (mutate existing entries)
		let match_arms: Vec<_> = overrides
			.iter()
			.map(|ovr| {
				let field_name_str = &ovr.field_name;
				let requirement_tokens = policy_kind_to_tokens(&ovr.policy, &conf_crate);
				quote! {
					#field_name_str => p.requirement = #requirement_tokens,
				}
			})
			.collect();

		// Generate insert statements for overrides not present in base policies.
		// This handles the case where `field_policies()` returns an empty slice
		// but the composition applies overrides that should still take effect.
		let insert_stmts: Vec<_> = overrides
			.iter()
			.map(|ovr| {
				let field_name_str = &ovr.field_name;
				let requirement_tokens = policy_kind_to_tokens(&ovr.policy, &conf_crate);
				let is_optional = matches!(ovr.policy, PolicyKind::Optional);
				quote! {
					if !policies.iter().any(|p| p.name == #field_name_str) {
						policies.push(#conf_crate::settings::policy::FieldPolicy {
							name: #field_name_str,
							requirement: #requirement_tokens,
							has_default: #is_optional,
						});
					}
				}
			})
			.collect();

		resolved_methods.push(quote! {
			/// Returns field policies for this fragment with composition-level overrides applied.
			fn #method_name() -> ::std::vec::Vec<#conf_crate::settings::policy::FieldPolicy> {
				let mut policies = <#type_path as #conf_crate::settings::fragment::SettingsFragment>::field_policies().to_vec();
				for p in &mut policies {
					match p.name {
						#(#match_arms)*
						_ => {}
					}
				}
				// Insert new entries for overrides targeting fields not in base policies
				#(#insert_stmts)*
				policies
			}
		});

		// Generate compile-time field existence assertion
		let field_access_checks: Vec<_> = overrides
			.iter()
			.map(|ovr| {
				let field_ident = format_ident!("{}", ovr.field_name);
				quote! {
					let _ = &_s.#field_ident;
				}
			})
			.collect();

		field_assertions.push(quote! {
			const _: () = {
				#[allow(unused)] // Compile-time field existence check, not runtime
				fn _assert_fields_exist(_s: &#type_path) {
					#(#field_access_checks)*
				}
			};
		});
	}

	// Generate ComposedSettings trait implementation
	//
	// For fragments WITH overrides, use the resolved_*_policies() method.
	// For fragments WITHOUT overrides, use field_policies() directly.
	//
	// Validation checks inside the section sub-map (e.g., merged["core"]["secret_key"])
	// rather than at the root level, matching the TOML `[section]` convention.
	let requirement_checks: Vec<_> = includes
		.iter()
		.map(|(key, type_name, overrides)| {
			let key_str = key.to_string();
			let type_path = resolve_fragment_type(type_name, &conf_crate);
			let policies_expr = if overrides.is_empty() {
				quote! {
					<#type_path as #conf_crate::settings::fragment::SettingsFragment>::field_policies()
				}
			} else {
				let method_name = format_ident!("resolved_{}_policies", key);
				quote! { &Self::#method_name() }
			};
			quote! {
				{
					// Look up the section sub-map (e.g., merged["core"])
					let section_map = merged.get(#key_str)
						.and_then(|v| v.as_object());
					for policy in #policies_expr {
						if policy.requirement == #conf_crate::settings::policy::FieldRequirement::Required {
							let found = section_map
								.map(|m| m.contains_key(policy.name))
								.unwrap_or(false);
							if !found {
								return ::std::result::Result::Err(#conf_crate::settings::builder::BuildError::MissingRequiredField {
									section: <#type_path as #conf_crate::settings::fragment::SettingsFragment>::section(),
									field: policy.name,
								});
							}
						}
					}
				}
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

		#(#field_assertions)*

		impl #struct_name {
			#(#resolved_methods)*

			/// Validate all fragments against the given profile.
			pub fn validate(
				&self,
				profile: &#conf_crate::settings::profile::Profile,
			) -> #conf_crate::settings::validation::ValidationResult {
				#(#validate_calls)*
				Ok(())
			}
		}

		impl #conf_crate::settings::composed::ComposedSettings for #struct_name {
			fn validate_requirements(
				merged: &#conf_crate::indexmap::IndexMap<::std::string::String, #conf_crate::serde_json::Value>,
			) -> ::std::result::Result<(), #conf_crate::settings::builder::BuildError> {
				#(#requirement_checks)*
				::std::result::Result::Ok(())
			}

			fn validate_fragments(
				&self,
				profile: &#conf_crate::settings::profile::Profile,
			) -> #conf_crate::settings::validation::ValidationResult {
				#(#validate_calls)*
				::std::result::Result::Ok(())
			}
		}
	})
}

/// Convert a parsed `PolicyKind` to its fully-qualified `FieldRequirement` token stream.
fn policy_kind_to_tokens(kind: &PolicyKind, conf_crate: &TokenStream) -> TokenStream {
	match kind {
		PolicyKind::Required => {
			quote! { #conf_crate::settings::policy::FieldRequirement::Required }
		}
		PolicyKind::Optional => {
			quote! { #conf_crate::settings::policy::FieldRequirement::Optional }
		}
	}
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
