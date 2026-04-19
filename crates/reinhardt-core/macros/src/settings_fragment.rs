//! Handler for `#[settings(fragment = true, section = "...")]`

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::spanned::Spanned;
use syn::{Fields, ItemStruct, LitStr, Result};

/// Parsed content of a `#[setting(...)]` field attribute.
#[derive(Debug)]
enum SettingAttr {
	/// `#[setting(required)]`
	Required,
	/// `#[setting(optional)]`
	Optional,
	/// `#[setting(default = "expr")]`
	Default(String),
}

/// Parse `#[setting(...)]` attributes from a single field.
///
/// Returns `None` if no `#[setting(...)]` attribute is present.
/// Returns a compile error for invalid combinations or unknown attributes.
fn parse_setting_attr(field: &syn::Field) -> Result<Option<SettingAttr>> {
	let mut result: Option<SettingAttr> = None;
	let mut has_required = false;
	let mut has_optional = false;
	let mut has_default = false;
	let mut default_expr: Option<String> = None;

	for attr in &field.attrs {
		if !attr.path().is_ident("setting") {
			continue;
		}

		attr.parse_nested_meta(|meta| {
			if meta.path.is_ident("required") {
				has_required = true;
				Ok(())
			} else if meta.path.is_ident("optional") {
				has_optional = true;
				Ok(())
			} else if meta.path.is_ident("default") {
				has_default = true;
				let lit: LitStr = meta.value()?.parse()?;
				default_expr = Some(lit.value());
				Ok(())
			} else {
				Err(meta.error(
					"unknown setting attribute, expected one of: `required`, `optional`, `default`",
				))
			}
		})?;
	}

	// Validate mutually exclusive combinations
	if has_required && has_default {
		let span = field
			.attrs
			.iter()
			.find(|a| a.path().is_ident("setting"))
			.map(|a| a.path().span())
			.unwrap_or_else(proc_macro2::Span::call_site);
		return Err(syn::Error::new(
			span,
			"`required` and `default` are mutually exclusive in `#[setting(...)]`",
		));
	}

	if has_required && has_optional {
		let span = field
			.attrs
			.iter()
			.find(|a| a.path().is_ident("setting"))
			.map(|a| a.path().span())
			.unwrap_or_else(proc_macro2::Span::call_site);
		return Err(syn::Error::new(
			span,
			"`required` and `optional` are mutually exclusive in `#[setting(...)]`",
		));
	}

	if has_required {
		result = Some(SettingAttr::Required);
	} else if has_default {
		result = Some(SettingAttr::Default(default_expr.unwrap()));
	} else if has_optional {
		result = Some(SettingAttr::Optional);
	}

	Ok(result)
}

/// Check if a field already has a `#[serde(default)]` or `#[serde(default = "...")]` attribute.
fn has_serde_default(field: &syn::Field) -> bool {
	for attr in &field.attrs {
		if !attr.path().is_ident("serde") {
			continue;
		}
		// Check if the serde attribute contains "default"
		let mut found = false;
		let _ = attr.parse_nested_meta(|meta| {
			if meta.path.is_ident("default") {
				found = true;
			}
			// Consume value if present (e.g., `default = "fn_name"`)
			if meta.path.is_ident("default") {
				let _ = meta.value().and_then(|v| v.parse::<LitStr>());
			}
			Ok(())
		});
		if found {
			return true;
		}
	}
	false
}

/// Strip `#[setting(...)]` attributes from field attrs, returning cleaned attrs.
fn strip_setting_attrs(attrs: &[syn::Attribute]) -> Vec<&syn::Attribute> {
	attrs
		.iter()
		.filter(|a| !a.path().is_ident("setting"))
		.collect()
}

/// Implementation for `#[settings(fragment = true, section = "...")]`.
pub(crate) fn settings_fragment_impl(args: TokenStream, input: ItemStruct) -> Result<TokenStream> {
	let conf_crate = crate::crate_paths::get_reinhardt_conf_crate();

	// Parse section, default_policy, and validate from args
	let mut section: Option<String> = None;
	let mut default_policy: Option<String> = None;
	let mut generate_validation: Option<bool> = None;

	let parser = syn::meta::parser(|meta| {
		if meta.path.is_ident("fragment") {
			let _: syn::LitBool = meta.value()?.parse()?;
			Ok(())
		} else if meta.path.is_ident("section") {
			let lit: LitStr = meta.value()?.parse()?;
			section = Some(lit.value());
			Ok(())
		} else if meta.path.is_ident("default_policy") {
			let lit: LitStr = meta.value()?.parse()?;
			let val = lit.value();
			if val != "required" && val != "optional" {
				return Err(syn::Error::new(
					lit.span(),
					"invalid `default_policy` value, expected `\"required\"` or `\"optional\"`",
				));
			}
			default_policy = Some(val);
			Ok(())
		} else if meta.path.is_ident("validate") {
			let lit: syn::LitBool = meta.value()?.parse()?;
			generate_validation = Some(lit.value());
			Ok(())
		} else {
			Err(meta.error(
				"expected `fragment = true`, `section = \"...\"`, `default_policy = \"...\"`, or `validate = true|false`",
			))
		}
	});

	syn::parse::Parser::parse2(parser, args)?;

	let section = section.ok_or_else(|| {
		syn::Error::new(
			proc_macro2::Span::call_site(),
			"`section = \"...\"` is required for `#[settings(fragment = true)]`",
		)
	})?;

	// Default policy: "optional" for backward compatibility
	let default_policy_is_required = default_policy.as_deref() == Some("required");

	// Whether to generate SettingsValidation impl (default: true)
	let should_generate_validation = generate_validation.unwrap_or(true);

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
	let semi_token = &input.semi_token;

	// Process fields: parse #[setting(...)] attrs, generate field_policies, strip attrs
	let mut field_policy_entries = Vec::new();
	let mut default_fn_defs = Vec::new();
	let mut new_fields = Vec::new();

	// Settings fragments must use named fields (braced structs)
	match &input.fields {
		Fields::Unnamed(unnamed) => {
			return Err(syn::Error::new(
				unnamed.paren_token.span.join(),
				"tuple structs are not supported for `#[settings(fragment = true)]`. \
				 Use a named-field struct instead.",
			));
		}
		Fields::Unit => {
			return Err(syn::Error::new(
				input.ident.span(),
				"unit structs are not supported for `#[settings(fragment = true)]`. \
				 Use a named-field struct instead.",
			));
		}
		Fields::Named(_) => {}
	}

	if let Fields::Named(ref named) = input.fields {
		for field in &named.named {
			let field_name = field.ident.as_ref().unwrap();
			let field_name_str = field_name.to_string();

			let setting_attr = parse_setting_attr(field)?;
			let already_has_serde_default = has_serde_default(field);

			// Determine requirement and has_default based on setting attr + default_policy
			let (requirement_tokens, has_default, serde_default_tokens) = match &setting_attr {
				Some(SettingAttr::Required) => (
					quote! { #conf_crate::settings::policy::FieldRequirement::Required },
					false,
					quote! {},
				),
				Some(SettingAttr::Optional) => {
					let serde_tokens = if already_has_serde_default {
						quote! {}
					} else {
						quote! { #[serde(default)] }
					};
					(
						quote! { #conf_crate::settings::policy::FieldRequirement::Optional },
						true,
						serde_tokens,
					)
				}
				Some(SettingAttr::Default(expr)) => {
					// Include struct name in generated function to avoid collisions
					// between multiple fragment structs in the same module
					let fn_name = format_ident!("__default_{}_{}", struct_name, field_name);
					let field_ty = &field.ty;
					let expr_tokens: TokenStream = expr.parse().map_err(|e| {
						syn::Error::new(
							field.ident.as_ref().unwrap().span(),
							format!("invalid default expression: {e}"),
						)
					})?;

					default_fn_defs.push(quote! {
						fn #fn_name() -> #field_ty {
							#expr_tokens
						}
					});

					let fn_name_str = fn_name.to_string();
					let serde_tokens = if already_has_serde_default {
						quote! {}
					} else {
						quote! { #[serde(default = #fn_name_str)] }
					};
					(
						quote! { #conf_crate::settings::policy::FieldRequirement::Optional },
						true,
						serde_tokens,
					)
				}
				None => {
					if default_policy_is_required {
						(
							quote! { #conf_crate::settings::policy::FieldRequirement::Required },
							false,
							quote! {},
						)
					} else {
						let serde_tokens = if already_has_serde_default {
							quote! {}
						} else {
							quote! { #[serde(default)] }
						};
						(
							quote! { #conf_crate::settings::policy::FieldRequirement::Optional },
							true,
							serde_tokens,
						)
					}
				}
			};

			field_policy_entries.push(quote! {
				#conf_crate::settings::policy::FieldPolicy {
					name: #field_name_str,
					requirement: #requirement_tokens,
					has_default: #has_default,
				}
			});

			// Rebuild field without #[setting(...)] attrs, with added serde default
			let cleaned_attrs = strip_setting_attrs(&field.attrs);
			let field_vis = &field.vis;
			let field_ty = &field.ty;

			new_fields.push(quote! {
				#(#cleaned_attrs)*
				#serde_default_tokens
				#field_vis #field_name: #field_ty
			});
		}
	}

	// Handle both named and unit structs
	let struct_body = if semi_token.is_some() {
		quote! { ; }
	} else {
		quote! {
			{
				#(#new_fields),*
			}
		}
	};

	let field_count = field_policy_entries.len();

	// Conditionally generate SettingsValidation impl and validate bridge.
	//
	// When `validate = true` (default): generate a no-op SettingsValidation impl.
	//   SettingsFragment uses its default no-op validate().
	// When `validate = false`: the user provides a custom SettingsValidation impl.
	//   Generate a SettingsFragment::validate() that delegates to SettingsValidation.
	let validation_impl = if should_generate_validation {
		quote! {
			impl #conf_crate::settings::fragment::SettingsValidation for #struct_name {}
		}
	} else {
		quote! {}
	};

	// When custom validation is provided (validate = false), bridge
	// SettingsFragment::validate to the user's SettingsValidation impl
	// so that callers using SettingsFragment::validate get custom logic.
	let validate_override = if !should_generate_validation {
		quote! {
			fn validate(
				&self,
				profile: &#conf_crate::settings::profile::Profile,
			) -> #conf_crate::settings::validation::ValidationResult {
				<Self as #conf_crate::settings::fragment::SettingsValidation>::validate(self, profile)
			}
		}
	} else {
		quote! {}
	};

	Ok(quote! {
		#derive_attr
		#(#attrs)*
		#vis struct #struct_name #struct_body

		#(#default_fn_defs)*

		#validation_impl

		impl #conf_crate::settings::fragment::SettingsFragment for #struct_name {
			type Accessor = dyn #trait_name;

			fn section() -> &'static str {
				#section
			}

			#validate_override

			fn field_policies() -> &'static [#conf_crate::settings::policy::FieldPolicy] {
				static POLICIES: [#conf_crate::settings::policy::FieldPolicy; #field_count] = [
					#(#field_policy_entries),*
				];
				&POLICIES
			}
		}

		/// Trait for accessing the settings fragment from a composed settings type.
		#vis trait #trait_name {
			/// Get a reference to the settings fragment.
			fn #method_name(&self) -> &#struct_name;
		}

		impl<T: #conf_crate::settings::fragment::HasSettings<#struct_name>> #trait_name for T {
			fn #method_name(&self) -> &#struct_name {
				self.get_settings()
			}
		}
	})
}
