//! Handler for `#[settings(fragment = true, section = "...")]`

use crate::settings_schema::{self, SettingAttr};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{ItemStruct, LitStr, Result};

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

	let parsed_fields = settings_schema::parse_fields(&input)?;

	// Default policy: "optional" for backward compatibility
	let default_policy_is_required = default_policy.as_deref() == Some("required");

	// Whether to generate SettingsValidation impl (default: true)
	let should_generate_validation = generate_validation.unwrap_or(true);

	let struct_name = &input.ident;
	let vis = &input.vis;

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
	let mut node_field_schema_entries = Vec::new();
	let mut default_fn_defs = Vec::new();
	let mut whole_field_check_defs = Vec::new();
	let mut new_fields = Vec::new();

	for (index, field) in parsed_fields.iter().enumerate() {
		let field_name = &field.ident;
		let field_name_str = &field.rust_name;
		let field_key_str = &field.key;
		let deserialize_keys = &field.deserialize_keys;
		let setting_attr = &field.setting_attr;
		let already_has_serde_default = field.has_serde_default;
		let cfg_attrs = &field.cfg_attrs;

		// Determine requirement and has_default based on setting attr + default_policy
		let (requirement_tokens, has_default, serde_default_tokens) = match setting_attr {
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
				let struct_fn_prefix = settings_schema::camel_to_snake(&struct_name.to_string());
				let fn_name = format_ident!("__default_{}_{}", struct_fn_prefix, field_name);
				let field_ty = &field.ty;
				let expr_tokens: TokenStream = expr.parse().map_err(|e| {
					syn::Error::new(
						field.ident.span(),
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

		let value_schema = settings_schema::value_schema_tokens(&field.shape, &conf_crate);
		let whole_field_check = if let Some((definition, check)) =
			settings_schema::whole_field_check_tokens(struct_name, field, index, &conf_crate)
		{
			whole_field_check_defs.push(definition);
			check
		} else {
			quote! { None }
		};

		if !field.skip_deserializing {
			field_policy_entries.push(quote! {
				#(#cfg_attrs)*
				#conf_crate::settings::policy::FieldPolicy {
					name: #field_name_str,
					requirement: #requirement_tokens,
					has_default: #has_default,
				}
			});

			node_field_schema_entries.push(quote! {
				#(#cfg_attrs)*
				#conf_crate::settings::schema::SettingsFieldSchema {
					rust_name: #field_name_str,
					key: #field_key_str,
					deserialize_keys: &[#(#deserialize_keys),*],
					policy: #conf_crate::settings::policy::FieldPolicy {
						name: #field_name_str,
						requirement: #requirement_tokens,
						has_default: #has_default,
					},
					whole_field_check: #whole_field_check,
					value: #value_schema,
				}
			});
		}

		// Rebuild field without #[setting(...)] attrs, with added serde default.
		let cleaned_attrs = &field.cleaned_attrs;
		let field_vis = &field.vis;
		let field_ty = &field.ty;

		new_fields.push(quote! {
			#(#cleaned_attrs)*
			#serde_default_tokens
			#field_vis #field_name: #field_ty
		});
	}

	let schema_name = settings_schema::schema_type_name(struct_name);
	let schema_parsed_fields = parsed_fields
		.iter()
		.filter(|field| !field.skip_deserializing)
		.cloned()
		.collect::<Vec<_>>();
	let schema_fields = settings_schema::schema_struct_fields(&schema_parsed_fields, &conf_crate);
	let schema_inits = settings_schema::schema_struct_inits(&schema_parsed_fields, &conf_crate);

	let schema_root_marker_field = if schema_fields.is_empty() {
		quote! {
			__root: ::std::marker::PhantomData<fn() -> Root>,
		}
	} else {
		quote! {}
	};

	let schema_root_marker_init = if schema_fields.is_empty() {
		quote! {
			__root: ::std::marker::PhantomData,
		}
	} else {
		quote! {}
	};

	// Rebuild the validated named-field struct.
	let struct_body = if semi_token.is_some() {
		quote! { ; }
	} else {
		quote! {
			{
				#(#new_fields),*
			}
		}
	};

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

	let root_fragment_impl = if let Some(section) = section {
		let trait_name = format_ident!("Has{}", struct_name);
		let method_name: syn::Ident = syn::parse_str(&section).map_err(|_| {
			syn::Error::new(
				struct_name.span(),
				format!(
					"`section` must be a valid Rust method identifier for the generated settings accessor; got `{section}`"
				),
			)
		})?;

		quote! {
			impl #conf_crate::settings::fragment::SettingsFragment for #struct_name {
				type Accessor = dyn #trait_name;

				fn section() -> &'static str {
					#section
				}

				#validate_override

				fn field_policies() -> &'static [#conf_crate::settings::policy::FieldPolicy] {
					static POLICIES: &[#conf_crate::settings::policy::FieldPolicy] = &[
						#(#field_policy_entries),*
					];
					POLICIES
				}
			}

			impl #conf_crate::settings::fragment::HasSettings<#struct_name> for #struct_name {
				fn get_settings(&self) -> &#struct_name {
					self
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
		}
	} else {
		quote! {}
	};

	Ok(quote! {
		#derive_attr
		#(#attrs)*
		#vis struct #struct_name #struct_body

			#(#default_fn_defs)*

			#(#whole_field_check_defs)*

		#validation_impl

		#[doc = "Typed schema references for this settings fragment."]
		#[derive(Clone, Debug)]
		#vis struct #schema_name<Root> {
			__path: #conf_crate::settings::schema::SettingsPathBuf,
			#schema_root_marker_field
			#(#schema_fields,)*
		}

		impl<Root> #schema_name<Root> {
			fn __from_path(path: #conf_crate::settings::schema::SettingsPathBuf) -> Self {
				Self {
					__path: path.clone(),
					#schema_root_marker_init
					#(#schema_inits,)*
				}
			}

			#[must_use]
			#[doc = "Return secret field references reachable from this settings fragment."]
			pub fn secret_fields(&self) -> ::std::vec::Vec<#conf_crate::settings::schema::SecretFieldRef<Root, ()>> {
				let mut paths = ::std::vec::Vec::new();
				<#struct_name as #conf_crate::settings::schema::SettingsNode>::node_schema()
					.collect_secret_paths(&mut paths);
				paths
					.into_iter()
					.map(|path| #conf_crate::settings::schema::SecretFieldRef::<Root, ()>::new(self.__path.clone().extend(path)))
					.collect()
			}
		}

		impl #conf_crate::settings::schema::SettingsNode for #struct_name {
			type Schema<Root> = #schema_name<Root>;

			fn schema_at<Root>(
				path: #conf_crate::settings::schema::SettingsPathBuf,
			) -> Self::Schema<Root> {
				#schema_name::__from_path(path)
			}

			fn node_schema() -> #conf_crate::settings::schema::SettingsNodeSchema {
				#conf_crate::settings::schema::SettingsNodeSchema {
					type_name: stringify!(#struct_name),
					fields: ::std::vec![#(#node_field_schema_entries),*],
				}
			}
		}

		#root_fragment_impl
	})
}
