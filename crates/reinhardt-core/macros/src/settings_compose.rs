//! Handler for `#[settings(key: Type | Type | key: Type)]`

use crate::settings_parser::{FieldOverride, FragmentEntry, PolicyKind, parse_settings_attr};
use crate::settings_schema;
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
	"MigrationSettings",
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

	// Collect includes with overrides; exclusion syntax is no longer supported.
	// The final boolean tracks type-only syntax so generated runtime code can
	// use the generated field name that Serde consumes, with the fragment
	// section retained as a compatibility fallback.
	let mut includes: Vec<(String, String, Vec<FieldOverride>, bool)> = vec![];
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
				includes.push((key.clone(), type_name.clone(), overrides.clone(), false));
			}
			FragmentEntry::TypeOnly(type_name) => {
				let key = settings_schema::infer_type_key(type_name)
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
				includes.push((key, type_name.clone(), vec![], true));
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
	// Each fragment field is deserialized from the generated composition key
	// (e.g., `[core]` → `core: CoreSettings`). Type-only composition retains
	// the fragment section as a metadata fallback when the inferred key is
	// absent, but Serde still consumes the generated field name.
	let field_defs: Vec<_> = includes
		.iter()
		.map(|(key, type_name, _, _)| {
			let key_ident = format_ident!("{}", key);
			let type_path = resolve_fragment_type(type_name, &conf_crate);
			quote! {
				pub #key_ident: #type_path
			}
		})
		.collect();

	let schema_name = settings_schema::schema_type_name(struct_name);
	let schema_field_defs: Vec<_> = includes
		.iter()
		.map(|(key, type_name, _, _)| {
			let key_ident = format_ident!("{}", key);
			let type_path = resolve_fragment_type(type_name, &conf_crate);
			quote! {
				#[doc = "Typed schema reference for this composed settings fragment."]
				pub #key_ident: <#type_path as #conf_crate::settings::schema::SettingsNode>::Schema<#struct_name>
			}
		})
		.collect();

	let schema_field_inits: Vec<_> = includes
		.iter()
		.map(|(key, type_name, _, _)| {
			let key_ident = format_ident!("{}", key);
			let key_str = key.as_str();
			let type_path = resolve_fragment_type(type_name, &conf_crate);
			let root_path = quote! {
				#conf_crate::settings::schema::SettingsPathBuf::from_key(#key_str)
			};
			quote! {
				#key_ident: <#type_path as #conf_crate::settings::schema::SettingsNode>::schema_at::<#struct_name>(#root_path)
			}
		})
		.collect();

	// Generate HasSettings<F> impls for each fragment
	let trait_impls: Vec<_> = includes
		.iter()
		.map(|(key, type_name, _, _)| {
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
		.map(|(key, _, _, _)| {
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

	for (key, type_name, overrides, _) in &includes {
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
		.map(|(key, type_name, overrides, is_type_only)| {
			let key_str = key.as_str();
			let type_path = resolve_fragment_type(type_name, &conf_crate);
			let primary_key_expr = quote! { #key_str };
			let fallback_key_expr = if *is_type_only {
				quote! { <#type_path as #conf_crate::settings::fragment::SettingsFragment>::section() }
			} else {
				quote! { #key_str }
			};
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
					let primary_key: &'static str = #primary_key_expr;
					let fallback_key: &'static str = #fallback_key_expr;
					let section_map = #conf_crate::settings::schema::root_section(
						merged,
						primary_key,
						fallback_key,
					);
					let section_path_key = if section_map.is_some()
						&& primary_key != fallback_key
						&& !merged.contains_key(primary_key)
					{
						fallback_key
					} else {
						primary_key
					};
					let mut node_schema = <#type_path as #conf_crate::settings::schema::SettingsNode>::node_schema();
					for policy in #policies_expr {
						if let Some(field_schema) = node_schema
							.fields
							.iter_mut()
							.find(|field| field.rust_name == policy.name)
						{
							field_schema.policy = *policy;
						}
					}
					for field_schema in &node_schema.fields {
						if field_schema.policy.requirement == #conf_crate::settings::policy::FieldRequirement::Required {
							let found = section_map
								.map(|m| {
									field_schema
										.deserialize_keys
										.iter()
										.any(|key| m.contains_key(*key))
								})
								.unwrap_or(false);
							if !found {
								return ::std::result::Result::Err(#conf_crate::settings::builder::BuildError::MissingRequiredField {
									section: section_path_key,
									field: field_schema.key,
								});
							}
						}
					}
					if let ::std::option::Option::Some(section_map) = section_map {
						node_schema.validate_required_map_at(
							section_map,
							#conf_crate::settings::schema::SettingsPathBuf::from_key(section_path_key),
						)?;
					}
				}
			}
		})
		.collect();

	let metadata_checks: Vec<_> = includes
		.iter()
		.map(|(key, type_name, overrides, is_type_only)| {
			let key_str = key.as_str();
			let type_path = resolve_fragment_type(type_name, &conf_crate);
			let primary_key_expr = quote! { #key_str };
			let fallback_key_expr = if *is_type_only {
				quote! { <#type_path as #conf_crate::settings::fragment::SettingsFragment>::section() }
			} else {
				quote! { #key_str }
			};
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
					let primary_key: &'static str = #primary_key_expr;
					let fallback_key: &'static str = #fallback_key_expr;
					let section_map = #conf_crate::settings::schema::root_section(
						merged,
						primary_key,
						fallback_key,
					);
					let section_path_key = if section_map.is_some()
						&& primary_key != fallback_key
						&& !merged.contains_key(primary_key)
					{
						fallback_key
					} else {
						primary_key
					};
					let mut node_schema = <#type_path as #conf_crate::settings::schema::SettingsNode>::node_schema();
					for policy in #policies_expr {
						if let Some(field_schema) = node_schema
							.fields
							.iter_mut()
							.find(|field| field.rust_name == policy.name)
						{
							let mut terminal_value = &field_schema.value;
							while let #conf_crate::settings::schema::SettingsValueSchema::Optional { inner } = terminal_value {
								terminal_value = inner;
							}
							if let #conf_crate::settings::schema::SettingsValueSchema::Leaf { .. } = terminal_value {
								field_schema.policy = *policy;
							}
						}
					}
					fields.extend(node_schema.resolve_fields(
						section_map,
						#conf_crate::settings::schema::SettingsPathBuf::from_key(section_path_key),
					));
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

		#[doc = "Typed schema references for this composed settings root."]
		#[derive(Clone, Debug)]
		#vis struct #schema_name {
			#(#schema_field_defs,)*
		}

		impl #conf_crate::settings::schema::HasSettingsSchema for #struct_name {
			type Schema = #schema_name;

			fn schema() -> Self::Schema {
				#schema_name {
					#(#schema_field_inits,)*
				}
			}
		}

		#(#trait_impls)*

		#(#field_assertions)*

		impl #struct_name {
			#(#resolved_methods)*

			/// Build typed schema references for this composed settings root.
			pub fn settings_schema() -> #schema_name {
				<Self as #conf_crate::settings::schema::HasSettingsSchema>::schema()
			}

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

			fn resolution_metadata(
				merged: &#conf_crate::indexmap::IndexMap<::std::string::String, #conf_crate::serde_json::Value>,
			) -> ::std::result::Result<#conf_crate::settings::schema::SettingsResolutionMetadata, #conf_crate::settings::builder::BuildError> {
				let mut fields = ::std::vec::Vec::new();
				#(#metadata_checks)*
				::std::result::Result::Ok(
					#conf_crate::settings::schema::SettingsResolutionMetadata::from_fields(fields)
				)
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
		let result = settings_schema::camel_to_snake(input);

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
		let result = settings_schema::infer_type_key(input);

		// Assert
		assert_eq!(result, expected.map(String::from).map_err(String::from));
	}

	#[rstest]
	#[case("MyCustomConfig", "does not end with `Settings`")]
	#[case("Settings", "empty prefix")]
	#[case("StaticSettings", "Rust keyword")]
	fn test_infer_field_name_error(#[case] input: &str, #[case] expected_contains: &str) {
		// Act
		let result = settings_schema::infer_type_key(input);

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
